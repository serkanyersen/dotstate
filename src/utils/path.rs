use std::path::{Path, PathBuf};

/// Get the home directory, with fallback to "/"
pub fn get_home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
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
    } else if path_str.starts_with("~/") {
        home_dir.join(&path_str[2..])
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

