use std::path::{Path, PathBuf};

/// Get the home directory, with fallback to "/"
pub fn get_home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Check if a path is a git repository
///
/// This is a simple check for the immediate directory.
/// For more robust detection (including nested repos), use `sync_validation::contains_git_repo`.
pub fn is_git_repo(path: &Path) -> bool {
    if path.is_dir() {
        path.join(".git").exists()
    } else {
        false
    }
}

/// Check if a path is safe to add as a custom file/folder
/// Returns (is_safe, reason_if_not_safe)
pub fn is_safe_to_add(path: &Path, repo_path: &Path) -> (bool, Option<String>) {
    let home_dir = get_home_dir();

    // Check if it's the home folder itself
    if path == home_dir {
        return (false, Some("Cannot add home folder".to_string()));
    }

    // Check if it's the root folder
    if path == Path::new("/") {
        return (false, Some("Cannot add root folder".to_string()));
    }

    // Check if it's the storage repo itself
    if path == repo_path {
        return (
            false,
            Some("Cannot add storage repository folder".to_string()),
        );
    }

    // Check if it's a parent of the storage repo
    if repo_path.strip_prefix(path).is_ok() {
        return (
            false,
            Some("Cannot add a parent folder of the storage repository".to_string()),
        );
    }

    // Check if storage repo is a parent of this path (this is actually OK, but we should warn)
    // Actually, this is fine - we can add files inside the repo

    (true, None)
}

/// Get the config directory path (always ~/.config/dotstate, regardless of OS)
pub fn get_config_dir() -> PathBuf {
    get_home_dir().join(".config").join("dotstate")
}

/// Get the config file path (always ~/.config/dotstate/config.toml, regardless of OS)
pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.toml")
}

/// Expand a path string, handling ~ and relative paths
///
/// # Arguments
/// * `path_str` - Path string that may contain ~ or be relative
///
/// # Returns
/// Expanded PathBuf
pub fn expand_path(path_str: &str) -> PathBuf {
    let home_dir = get_home_dir();

    if path_str.starts_with('/') {
        PathBuf::from(path_str)
    } else if let Some(stripped) = path_str.strip_prefix("~/") {
        home_dir.join(stripped)
    } else if path_str == "~" {
        home_dir
    } else {
        // Relative path - join with home directory
        home_dir.join(path_str)
    }
}

/// Format a path for display (shorten if too long, show ~ for home)
///
/// # Arguments
/// * `path` - Path to format
///
/// # Returns
/// Formatted string
#[allow(dead_code)]
pub fn format_path_for_display(path: &Path) -> String {
    let home_dir = get_home_dir();

    if let Ok(relative) = path.strip_prefix(&home_dir) {
        if relative.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~/{}", relative.to_string_lossy())
        }
    } else {
        path.to_string_lossy().to_string()
    }
}

/// Check if a path is a dotfile (starts with .)
///
/// # Arguments
/// * `path` - Path to check
///
/// # Returns
/// True if the path represents a dotfile
#[allow(dead_code)]
pub fn is_dotfile(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false)
}

/// Get the repository path from the config file
///
/// # Returns
/// PathBuf to the repository directory
pub fn get_repository_path() -> anyhow::Result<PathBuf> {
    use crate::config::Config;
    let config_path = get_config_path();
    let config = Config::load_or_create(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;
    Ok(config.repo_path)
}
