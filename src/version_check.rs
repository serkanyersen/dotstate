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
pub fn check_for_updates(_interval_hours: u64) -> Option<UpdateInfo> {
    // Always do a fresh check - the TUI only calls this once at startup anyway
    // Using Duration::ZERO bypasses the "first run" caching behavior
    check_for_updates_now()
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

    // Use Duration::ZERO to skip cache and force a fresh check every time
    let informer =
        update_informer::new(GitHub, &repo, current_version).interval(Duration::ZERO);

    match informer.check_version() {
        Ok(Some(new_version)) => {
            let version_str = new_version.to_string();
            // version_str already includes 'v' prefix from GitHub tags
            Some(UpdateInfo {
                current_version: current_version.to_string(),
                latest_version: version_str.clone(),
                release_url: format!(
                    "https://github.com/{}/{}/releases/tag/{}",
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
    fn test_current_version_matches_cargo() {
        let version = current_version();
        assert_eq!(version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn test_install_script_url() {
        let url = UpdateInfo::install_script_url();
        assert!(url.starts_with("https://"));
        assert!(url.contains("install.sh"));
        assert!(url.contains("dotstate"));
    }

    #[test]
    fn test_releases_url() {
        let url = UpdateInfo::releases_url();
        assert!(url.contains("github.com"));
        assert!(url.contains("releases"));
        assert!(url.contains(REPO_OWNER));
        assert!(url.contains(REPO_NAME));
    }

    #[test]
    fn test_update_info_creation() {
        let info = UpdateInfo {
            current_version: "1.0.0".to_string(),
            latest_version: "2.0.0".to_string(),
            release_url: "https://github.com/test/repo/releases/tag/v2.0.0".to_string(),
        };

        assert_eq!(info.current_version, "1.0.0");
        assert_eq!(info.latest_version, "2.0.0");
        assert!(info.release_url.contains("v2.0.0"));
    }

    #[test]
    fn test_update_info_clone() {
        let info = UpdateInfo {
            current_version: "1.0.0".to_string(),
            latest_version: "2.0.0".to_string(),
            release_url: "https://example.com".to_string(),
        };

        let cloned = info.clone();
        assert_eq!(info.current_version, cloned.current_version);
        assert_eq!(info.latest_version, cloned.latest_version);
        assert_eq!(info.release_url, cloned.release_url);
    }

    #[test]
    fn test_repo_constants() {
        assert_eq!(REPO_OWNER, "serkanyersen");
        assert_eq!(REPO_NAME, "dotstate");
    }

    #[test]
    fn test_releases_url_format() {
        let url = UpdateInfo::releases_url();
        let expected = format!("https://github.com/{}/{}/releases", REPO_OWNER, REPO_NAME);
        assert_eq!(url, expected);
    }
}
