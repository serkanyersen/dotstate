//! Version checking module for DotState
//!
//! This module handles checking for new versions of DotState from GitHub releases
//! and provides update information to users.

use std::time::Duration;
use update_informer::{registry::GitHub, Check};

/// GitHub repository owner
const REPO_OWNER: &str = "serkanyersen";
/// GitHub repository name
const REPO_NAME: &str = "dotstate";

/// Information about an available update
#[derive(Debug, Clone)]
pub struct UpdateInfo {
    /// Current installed version
    pub current_version: String,
    /// Latest available version
    pub latest_version: String,
    /// URL to the release page
    pub release_url: String,
}

impl UpdateInfo {
    /// Get the install.sh URL for self-update
    pub fn install_script_url() -> &'static str {
        "https://dotstate.serkan.dev/install.sh"
    }

    /// Get the GitHub releases URL
    pub fn releases_url() -> String {
        format!("https://github.com/{}/{}/releases", REPO_OWNER, REPO_NAME)
    }
}

/// Check for updates using update-informer
///
/// This function respects the check interval configured by the user.
/// Results are cached by update-informer to prevent excessive API calls.
///
/// # Arguments
/// * `interval_hours` - How often to check for updates (in hours)
///
/// # Returns
/// * `Some(UpdateInfo)` if a newer version is available
/// * `None` if already up to date or check failed/skipped
pub fn check_for_updates(interval_hours: u64) -> Option<UpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION");
    let repo = format!("{}/{}", REPO_OWNER, REPO_NAME);

    let informer = update_informer::new(GitHub, &repo, current_version)
        .interval(Duration::from_secs(interval_hours * 60 * 60));

    match informer.check_version() {
        Ok(Some(new_version)) => {
            let version_str = new_version.to_string();
            Some(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: version_str.clone(),
                release_url: format!(
                    "https://github.com/{}/{}/releases/tag/v{}",
                    REPO_OWNER, REPO_NAME, version_str
                ),
            })
        }
        Ok(None) => None, // Already up to date
        Err(e) => {
            // Log error but don't fail - update checks should be non-blocking
            tracing::debug!("Failed to check for updates: {}", e);
            None
        }
    }
}

/// Force check for updates, ignoring the cache
///
/// This is useful for the `dotstate upgrade` command where the user
/// explicitly wants to check for updates.
///
/// # Returns
/// * `Some(UpdateInfo)` if a newer version is available
/// * `None` if already up to date or check failed
pub fn check_for_updates_now() -> Option<UpdateInfo> {
    let current_version = env!("CARGO_PKG_VERSION");
    let repo = format!("{}/{}", REPO_OWNER, REPO_NAME);

    // Use Duration::ZERO to disable caching and force a fresh check
    let informer = update_informer::new(GitHub, &repo, current_version).interval(Duration::ZERO);

    match informer.check_version() {
        Ok(Some(new_version)) => {
            let version_str = new_version.to_string();
            Some(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: version_str.clone(),
                release_url: format!(
                    "https://github.com/{}/{}/releases/tag/v{}",
                    REPO_OWNER, REPO_NAME, version_str
                ),
            })
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("Failed to check for updates: {}", e);
            None
        }
    }
}

/// Get the current version of DotState
pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version() {
        let version = current_version();
        assert!(!version.is_empty());
        // Should be a valid semver
        assert!(version.contains('.'));
    }

    #[test]
    fn test_install_script_url() {
        let url = UpdateInfo::install_script_url();
        assert!(url.starts_with("https://"));
        assert!(url.contains("install.sh"));
    }

    #[test]
    fn test_releases_url() {
        let url = UpdateInfo::releases_url();
        assert!(url.contains("github.com"));
        assert!(url.contains("releases"));
    }
}
