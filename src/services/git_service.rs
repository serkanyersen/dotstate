//! Git service for repository operations.
//!
//! This module provides a service layer for git operations, abstracting
//! the details of the git implementation from the UI layer.

use crate::config::{Config, RepoMode};
use crate::git::GitManager;
use anyhow::Result;
use std::path::Path;
use tracing::warn;

/// Result of checking for changes that need to be pushed.
#[derive(Debug, Clone, Default)]
pub struct ChangesCheckResult {
    /// Whether there are any changes to push.
    pub has_changes: bool,
    /// List of changed files with their status (e.g., "M filename").
    pub changed_files: Vec<String>,
}

/// Result of a sync operation.
#[derive(Debug)]
pub struct SyncResult {
    /// Whether the sync was successful.
    pub success: bool,
    /// Message describing the result.
    pub message: String,
    /// Number of changes pulled from remote (if any).
    pub pulled_count: Option<usize>,
}

/// Detailed status of the git repository.
#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    /// Whether there are any uncommitted changes.
    pub has_changes: bool,
    /// List of uncommitted changed files.
    pub uncommitted_files: Vec<String>,
    /// Number of commits ahead of remote.
    pub ahead: usize,
    /// Number of commits behind remote.
    pub behind: usize,
    /// Any error message encountered during check.
    pub error: Option<String>,
}

/// Service for git-related operations.
///
/// This service provides a clean interface for git operations without
/// direct dependencies on UI state.
pub struct GitService;

impl GitService {
    /// Check for changes that need to be pushed to remote.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration containing repo settings.
    ///
    /// # Returns
    ///
    /// A `ChangesCheckResult` containing information about pending changes.
    #[must_use]
    pub fn check_changes_to_push(config: &Config) -> ChangesCheckResult {
        let mut result = ChangesCheckResult::default();

        // Check if repository is configured and repo exists
        if !config.is_repo_configured() {
            return result;
        }

        let repo_path = &config.repo_path;
        if !repo_path.exists() {
            return result;
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(_) => return result,
        };

        // Get changed files (this includes both uncommitted and unpushed)
        if let Ok(files) = git_mgr.get_changed_files() {
            result.has_changes = !files.is_empty();
            result.changed_files = files;
        } else {
            // Fallback to old method if get_changed_files fails
            // Check for uncommitted changes
            let has_uncommitted = git_mgr.has_uncommitted_changes().unwrap_or(false);

            // Check for unpushed commits
            let branch = git_mgr
                .get_current_branch()
                .unwrap_or_else(|| "main".to_string());
            let has_unpushed = git_mgr
                .has_unpushed_commits("origin", &branch)
                .unwrap_or(false);

            result.has_changes = has_uncommitted || has_unpushed;
        }

        result
    }

    /// Fetch updates and check comprehensive status (uncommitted + ahead/behind).
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    ///
    /// # Returns
    ///
    /// A `GitStatus` with detailed repository state.
    pub fn fetch_and_check_status(config: &Config) -> GitStatus {
        let mut status = GitStatus::default();

        // Check if repository is configured and repo exists
        if !config.is_repo_configured() {
            return status;
        }

        let repo_path = &config.repo_path;
        if !repo_path.exists() {
            status.error = Some("Repository path does not exist".to_string());
            return status;
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(e) => {
                status.error = Some(format!("Failed to open repository: {e}"));
                return status;
            }
        };

        // 1. Check uncommitted changes
        match git_mgr.get_changed_files() {
            Ok(files) => {
                status.has_changes = !files.is_empty();
                status.uncommitted_files = files;
            }
            Err(e) => {
                status.error = Some(format!("Failed to check changes: {e}"));
                return status;
            }
        }

        // 2. Fetch and check ahead/behind (if repo has a remote)
        // For GitHub mode, require token. For Local mode, try without token (SSH or public repo).
        let branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| config.default_branch.clone());

        // Check if repo has a remote configured
        let has_remote = git_mgr.has_remote("origin");

        if has_remote {
            let token = match config.repo_mode {
                RepoMode::Local => None, // Local repos use SSH or no auth
                RepoMode::GitHub => config.get_github_token(),
            };

            // Only require token for GitHub mode
            let should_fetch = match config.repo_mode {
                RepoMode::Local => true, // Always try for local repos with remotes
                RepoMode::GitHub => token.is_some(),
            };

            if should_fetch {
                // Try to fetch
                if let Err(e) = git_mgr.fetch("origin", &branch, token.as_deref()) {
                    warn!("Background fetch failed: {}", e);
                    // Don't fail the whole status check, just record error
                    // We can still return uncommitted changes info
                }
            }

            // Check ahead/behind counts (even if fetch failed, we might have cached data)
            match git_mgr.get_ahead_behind("origin", &branch) {
                Ok((ahead, behind)) => {
                    status.ahead = ahead;
                    status.behind = behind;
                }
                Err(e) => {
                    warn!("Failed to get ahead/behind count: {}", e);
                }
            }
        }

        status
    }

    /// Load changed files from the repository.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository.
    ///
    /// # Returns
    ///
    /// A vector of changed file descriptions.
    #[must_use]
    pub fn load_changed_files(repo_path: &Path) -> Vec<String> {
        if !repo_path.exists() {
            return vec![];
        }

        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(_) => return vec![],
        };

        git_mgr.get_changed_files().unwrap_or_default()
    }

    /// Get the diff for a specific file.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the git repository.
    /// * `file_info` - File info string in format "X filename" where X is the status.
    ///
    /// # Returns
    ///
    /// The diff content if available.
    #[must_use]
    pub fn get_diff_for_file(repo_path: &Path, file_info: &str) -> Option<String> {
        // Format is "X filename"
        let parts: Vec<&str> = file_info.splitn(2, ' ').collect();
        if parts.len() != 2 {
            return None;
        }
        let path_str = parts[1].trim();

        let git_mgr = GitManager::open_or_init(repo_path).ok()?;
        git_mgr.get_diff_for_file(path_str).ok().flatten()
    }

    /// Perform a sync operation: commit -> pull with rebase -> push.
    ///
    /// # Arguments
    ///
    /// * `config` - Application configuration.
    ///
    /// # Returns
    ///
    /// A `SyncResult` describing the outcome of the operation.
    pub fn sync(config: &Config) -> SyncResult {
        // Check if repository is configured
        if !config.is_repo_configured() {
            warn!("Sync attempted but repository not configured");
            return SyncResult {
                success: false,
                message: "Error: Repository not configured.\n\n\
                    Please set up your repository first from the main menu."
                    .to_string(),
                pulled_count: None,
            };
        }

        let repo_path = &config.repo_path;

        // Check if repo exists
        if !repo_path.exists() {
            warn!("Sync attempted but repository not found: {:?}", repo_path);
            return SyncResult {
                success: false,
                message: format!(
                    "Error: Repository not found at {repo_path:?}\n\n\
                    Please sync some files first."
                ),
                pulled_count: None,
            };
        }

        // Open git repository
        let git_mgr = match GitManager::open_or_init(repo_path) {
            Ok(mgr) => mgr,
            Err(e) => {
                return SyncResult {
                    success: false,
                    message: format!("Error: Failed to open repository: {e}"),
                    pulled_count: None,
                }
            }
        };

        let branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| config.default_branch.clone());

        // Get token based on repo mode
        let token_string = match config.repo_mode {
            RepoMode::Local => None,
            RepoMode::GitHub => config.get_github_token(),
        };
        let token = token_string.as_deref();

        // Only require token for GitHub mode
        if matches!(config.repo_mode, RepoMode::GitHub) && token.is_none() {
            return SyncResult {
                success: false,
                message: "Error: GitHub token not found.\n\n\
                    Please provide a GitHub token using one of these methods:\n\n\
                    1. Set the DOTSTATE_GITHUB_TOKEN environment variable:\n\
                       export DOTSTATE_GITHUB_TOKEN=ghp_your_token_here\n\n\
                    2. Configure it in the TUI by going to the main menu\n\n\
                    Create a token at: https://github.com/settings/tokens\n\
                    Required scope: repo (full control of private repositories)"
                    .to_string(),
                pulled_count: None,
            };
        }

        // Step 1: Only commit if there are uncommitted changes
        // This prevents creating empty commits on retry after a failed push
        let has_changes = git_mgr.has_uncommitted_changes().unwrap_or(false);

        if has_changes {
            let commit_msg = git_mgr
                .generate_commit_message()
                .unwrap_or_else(|_| "Update dotfiles".to_string());

            if let Err(e) = git_mgr.commit_all(&commit_msg) {
                return SyncResult {
                    success: false,
                    message: Self::format_error_chain("Failed to commit changes", &e),
                    pulled_count: None,
                };
            }
        }

        // Step 2: Pull with rebase
        let pulled_count = match git_mgr.pull_with_rebase("origin", &branch, token) {
            Ok(count) => count,
            Err(e) => {
                return SyncResult {
                    success: false,
                    message: Self::format_error_chain("Failed to pull from remote", &e),
                    pulled_count: None,
                }
            }
        };

        // Step 3: Push to remote
        if let Err(e) = git_mgr.push("origin", &branch, token) {
            return SyncResult {
                success: false,
                message: Self::format_error_chain("Failed to push to remote", &e),
                pulled_count: Some(pulled_count),
            };
        }

        // Success! Build the success message
        let mut success_msg = format!(
            "✓ Successfully synced with remote!\n\n\
            Branch: {branch}\n\
            Repository: {repo_path:?}"
        );

        if pulled_count > 0 {
            success_msg.push_str(&format!("\n\nPulled {pulled_count} change(s) from remote."));

            // Step 4: Ensure symlinks for any new files pulled from remote
            // This is efficient - only creates symlinks for missing files
            use crate::services::ProfileService;
            match ProfileService::ensure_profile_symlinks(
                repo_path,
                &config.active_profile,
                config.backup_enabled,
            ) {
                Ok((created, _skipped, errors)) => {
                    if created > 0 {
                        success_msg
                            .push_str(&format!("\nCreated {created} symlink(s) for new files."));
                    }
                    if !errors.is_empty() {
                        success_msg.push_str(&format!(
                            "\n\nWarning: {} error(s) creating symlinks:\n{}",
                            errors.len(),
                            errors.join("\n")
                        ));
                    }
                }
                Err(e) => {
                    warn!("Failed to ensure symlinks after pull: {}", e);
                    success_msg.push_str(&format!(
                        "\n\nWarning: Failed to create symlinks for new files: {e}"
                    ));
                }
            }

            // Also ensure common symlinks
            match ProfileService::ensure_common_symlinks(repo_path, config.backup_enabled) {
                Ok((created, _skipped, errors)) => {
                    if created > 0 {
                        success_msg.push_str(&format!("\nCreated {created} common symlink(s)."));
                    }
                    if !errors.is_empty() {
                        success_msg.push_str(&format!(
                            "\n\nWarning: {} error(s) creating common symlinks:\n{}",
                            errors.len(),
                            errors.join("\n")
                        ));
                    }
                }
                Err(e) => {
                    warn!("Failed to ensure common symlinks after pull: {}", e);
                    success_msg.push_str(&format!(
                        "\n\nWarning: Failed to create common symlinks: {e}"
                    ));
                }
            }
        } else {
            success_msg.push_str("\n\nNo changes pulled from remote.");
        }

        SyncResult {
            success: true,
            message: success_msg,
            pulled_count: Some(pulled_count),
        }
    }

    /// Format an error with its full chain for display.
    fn format_error_chain(context: &str, error: &anyhow::Error) -> String {
        let mut msg = format!("Error: {context}: {error}");
        for cause in error.chain().skip(1) {
            msg.push_str(&format!("\n  Caused by: {cause}"));
        }
        msg
    }

    /// Clone or open a repository.
    ///
    /// # Arguments
    ///
    /// * `remote_url` - The remote URL to clone from.
    /// * `local_path` - The local path to clone to.
    /// * `token` - Optional authentication token.
    ///
    /// # Returns
    ///
    /// A tuple of (`GitManager`, `was_existing`).
    pub fn clone_or_open(
        remote_url: &str,
        local_path: &Path,
        token: Option<&str>,
    ) -> Result<(GitManager, bool)> {
        GitManager::clone_or_open(remote_url, local_path, token)
    }

    /// Initialize a new repository or open existing one.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to initialize or open.
    ///
    /// # Returns
    ///
    /// A `GitManager` instance.
    pub fn open_or_init(path: &Path) -> Result<GitManager> {
        GitManager::open_or_init(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_check_changes_unconfigured() {
        let config = Config::default();
        let result = GitService::check_changes_to_push(&config);
        assert!(!result.has_changes);
        assert!(result.changed_files.is_empty());
    }

    #[test]
    fn test_load_changed_files_nonexistent() {
        let result = GitService::load_changed_files(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_get_diff_invalid_format() {
        let result = GitService::get_diff_for_file(&PathBuf::from("/tmp"), "invalid");
        assert!(result.is_none());
    }
}
