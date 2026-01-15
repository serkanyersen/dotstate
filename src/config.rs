use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Repository setup mode
/// Determines how the repository was configured and how sync operations authenticate
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum RepoMode {
    /// Repository created/managed via GitHub API (requires token for sync)
    #[default]
    GitHub,
    /// User-provided repository (uses system git credentials for sync)
    Local,
}

/// Update check configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateConfig {
    /// Whether to check for updates on startup (default: true)
    #[serde(default = "default_update_check_enabled")]
    pub check_enabled: bool,
    /// Check interval in hours (default: 24)
    #[serde(default = "default_update_check_interval")]
    pub check_interval_hours: u64,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            check_enabled: default_update_check_enabled(),
            check_interval_hours: default_update_check_interval(),
        }
    }
}

fn default_update_check_enabled() -> bool {
    true
}

fn default_update_check_interval() -> u64 {
    24
}

/// Main configuration structure
/// Note: Profiles are stored in the repository manifest (.dotstate-profiles.toml), not in this config file.
/// This config only stores local settings like backup preferences and active profile name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Repository setup mode (GitHub API or local/user-provided)
    #[serde(default)]
    pub repo_mode: RepoMode,
    /// GitHub repository information (only used in GitHub mode)
    pub github: Option<GitHubConfig>,
    /// Current active profile/set
    pub active_profile: String,
    /// Repository root path (where dotfiles are stored locally)
    pub repo_path: PathBuf,
    /// Repository name on GitHub (default: dotstate-storage)
    #[serde(default = "default_repo_name")]
    pub repo_name: String,
    /// Default branch name (default: main)
    #[serde(default = "default_branch_name")]
    pub default_branch: String,
    /// Whether to create backups before syncing (default: true)
    #[serde(default = "default_backup_enabled")]
    pub backup_enabled: bool,
    /// Whether the active profile is currently activated (symlinks created)
    #[serde(default)]
    pub profile_activated: bool,
    /// Custom file paths that the user has added (persists even if removed from sync)
    #[serde(default)]
    pub custom_files: Vec<String>,
    /// Update check configuration
    #[serde(default)]
    pub updates: UpdateConfig,
    /// Color theme: "dark", "light", or "nocolor" (default: dark)
    #[serde(default = "default_theme")]
    pub theme: String,
    /// Icon set: "nerd", "unicode", or "ascii" (default: auto-detect)
    #[serde(default = "default_icon_set")]
    pub icon_set: String,
    /// Keymap configuration (preset and overrides)
    #[serde(default)]
    pub keymap: crate::keymap::Keymap,
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_icon_set() -> String {
    "auto".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubConfig {
    /// Repository owner (username or org)
    pub owner: String,
    /// Repository name
    pub repo: String,
    /// OAuth token or PAT
    pub token: Option<String>,
}

// Profile struct removed - profiles are now stored in the repository manifest (.dotstate-profiles.toml)
// Use crate::utils::ProfileManifest and ProfileInfo instead

pub fn default_repo_name() -> String {
    "dotstate-storage".to_string()
}

fn default_branch_name() -> String {
    "main".to_string()
}

fn default_backup_enabled() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            repo_mode: RepoMode::default(),
            github: None,
            active_profile: String::new(),
            backup_enabled: true,
            profile_activated: true,
            repo_path: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".config")
                .join("dotstate")
                .join("storage"),
            repo_name: default_repo_name(),
            default_branch: "main".to_string(),
            custom_files: Vec::new(),
            updates: UpdateConfig::default(),
            theme: default_theme(),
            icon_set: default_icon_set(),
            keymap: crate::keymap::Keymap::default(),
        }
    }
}

impl Config {
    /// Load configuration from file or create default
    /// If config doesn't exist, attempts to discover profiles from the repo manifest
    pub fn load_or_create(config_path: &Path) -> Result<Self> {
        if config_path.exists() {
            tracing::debug!("Loading config from: {:?}", config_path);
            let content = std::fs::read_to_string(config_path)
                .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
            let mut config: Config =
                toml::from_str(&content).with_context(|| "Failed to parse config file")?;

            // Set defaults for missing fields (for backward compatibility)
            if config.repo_name.is_empty() {
                config.repo_name = default_repo_name();
            }
            if config.default_branch.is_empty() {
                config.default_branch = default_branch_name();
            }
            // backup_enabled defaults to true if not present
            // (handled by serde default)

            // If active_profile is empty and repo exists, try to set it from manifest
            if config.active_profile.is_empty() && config.repo_path.exists() {
                if let Ok(manifest) =
                    crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
                {
                    if let Some(first_profile) = manifest.profiles.first() {
                        config.active_profile = first_profile.name.clone();
                        config.save(config_path)?;
                    }
                }
            }

            tracing::info!("Config loaded successfully");
            Ok(config)
        } else {
            // Config doesn't exist - create default
            tracing::info!(
                "Config not found, creating default config at: {:?}",
                config_path
            );
            let mut config = Self::default();

            // Try to discover active profile from the repo manifest if repo_path exists
            if config.repo_path.exists() {
                if let Ok(manifest) =
                    crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
                {
                    if let Some(first_profile) = manifest.profiles.first() {
                        config.active_profile = first_profile.name.clone();
                    }
                }
            }

            config.save(config_path)?;
            Ok(config)
        }
    }

    /// Save configuration to file with secure permissions
    pub fn save(&self, config_path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).with_context(|| "Failed to serialize config")?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory: {:?}", parent))?;
        }

        // Write file
        std::fs::write(config_path, content)
            .with_context(|| format!("Failed to write config file: {:?}", config_path))?;

        // Set secure permissions (600: owner read/write only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(config_path)
                .with_context(|| format!("Failed to get file metadata: {:?}", config_path))?
                .permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(config_path, perms)
                .with_context(|| format!("Failed to set file permissions: {:?}", config_path))?;
        }

        Ok(())
    }

    /// Check if the repository is configured (either GitHub or Local mode)
    pub fn is_repo_configured(&self) -> bool {
        match self.repo_mode {
            RepoMode::GitHub => self.github.is_some(),
            RepoMode::Local => self.repo_path.join(".git").exists(),
        }
    }

    /// Get GitHub token from environment variable or config
    /// Priority: DOTSTATE_GITHUB_TOKEN env var > config token
    /// Returns None if neither is set
    pub fn get_github_token(&self) -> Option<String> {
        // First, check environment variable
        if let Ok(token) = std::env::var("DOTSTATE_GITHUB_TOKEN") {
            if !token.is_empty() {
                tracing::debug!(
                    "Using GitHub token from DOTSTATE_GITHUB_TOKEN environment variable"
                );
                return Some(token);
            }
        }

        // Fall back to config token
        self.github
            .as_ref()
            .and_then(|gh| gh.token.as_ref())
            .cloned()
    }

    /// Get the icon set based on config value
    /// Returns the configured icon set, or auto-detects if set to "auto"
    pub fn get_icon_set(&self) -> crate::icons::IconSet {
        use crate::icons::IconSet;

        match self.icon_set.to_lowercase().as_str() {
            "nerd" | "nerdfont" | "nerdfonts" => IconSet::NerdFonts,
            "unicode" | "emoji" => IconSet::Unicode,
            "ascii" | "plain" => IconSet::Ascii,
            _ => IconSet::detect(), // Auto-detect or fallback to detection
        }
    }

    // Profile-related methods removed - use ProfileManifest directly
    // Helper method removed as it's not used - profiles are accessed via App::get_profiles() instead
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.active_profile, "");
        assert_eq!(config.repo_mode, RepoMode::GitHub);
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Create a config with a non-existent repo_path to avoid manifest loading
        let config = Config {
            repo_path: repo_path.clone(),
            ..Default::default()
        };
        config.save(&config_path).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        // Both should have empty active_profile since repo_path doesn't exist
        assert_eq!(config.active_profile, loaded.active_profile);
        assert_eq!(loaded.active_profile, "");
    }

    #[test]
    fn test_repo_mode_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Test GitHub mode
        let config = Config {
            repo_path: repo_path.clone(),
            repo_mode: RepoMode::GitHub,
            ..Default::default()
        };
        config.save(&config_path).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        assert_eq!(loaded.repo_mode, RepoMode::GitHub);

        // Test Local mode
        let config = Config {
            repo_path: repo_path.clone(),
            repo_mode: RepoMode::Local,
            ..Default::default()
        };
        config.save(&config_path).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        assert_eq!(loaded.repo_mode, RepoMode::Local);
    }

    #[test]
    fn test_old_config_defaults_to_github_mode() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Write an old-style config without repo_mode
        let old_config = format!(
            r#"
active_profile = ""
repo_path = "{}"
repo_name = "dotstate-storage"
default_branch = "main"
backup_enabled = true
profile_activated = true
custom_files = []
"#,
            repo_path.display()
        );
        std::fs::write(&config_path, old_config).unwrap();

        // Load should default repo_mode to GitHub
        let loaded = Config::load_or_create(&config_path).unwrap();
        assert_eq!(loaded.repo_mode, RepoMode::GitHub);
    }

    #[test]
    fn test_update_config_defaults() {
        let update_config = UpdateConfig::default();
        assert!(update_config.check_enabled);
        assert_eq!(update_config.check_interval_hours, 24);
    }

    #[test]
    fn test_config_includes_update_config() {
        let config = Config::default();
        assert!(config.updates.check_enabled);
        assert_eq!(config.updates.check_interval_hours, 24);
    }

    #[test]
    fn test_update_config_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        let mut config = Config {
            repo_path,
            ..Default::default()
        };
        config.updates.check_enabled = false;
        config.updates.check_interval_hours = 48;
        config.save(&config_path).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        assert!(!loaded.updates.check_enabled);
        assert_eq!(loaded.updates.check_interval_hours, 48);
    }

    #[test]
    fn test_old_config_defaults_update_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Write an old-style config without updates section
        let old_config = format!(
            r#"
active_profile = ""
repo_path = "{}"
repo_name = "dotstate-storage"
default_branch = "main"
backup_enabled = true
profile_activated = true
custom_files = []
"#,
            repo_path.display()
        );
        std::fs::write(&config_path, old_config).unwrap();

        // Load should default updates to enabled with 24h interval
        let loaded = Config::load_or_create(&config_path).unwrap();
        assert!(loaded.updates.check_enabled);
        assert_eq!(loaded.updates.check_interval_hours, 24);
    }

    #[test]
    fn test_update_config_custom_interval() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Write config with custom update interval
        let config_content = format!(
            r#"
active_profile = ""
repo_path = "{}"
repo_name = "dotstate-storage"
default_branch = "main"
backup_enabled = true
profile_activated = true
custom_files = []

[updates]
check_enabled = true
check_interval_hours = 168
"#,
            repo_path.display()
        );
        std::fs::write(&config_path, config_content).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        assert!(loaded.updates.check_enabled);
        assert_eq!(loaded.updates.check_interval_hours, 168); // 1 week
    }

    #[test]
    fn test_update_config_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Write config with updates disabled
        let config_content = format!(
            r#"
active_profile = ""
repo_path = "{}"
repo_name = "dotstate-storage"
default_branch = "main"
backup_enabled = true
profile_activated = true
custom_files = []

[updates]
check_enabled = false
"#,
            repo_path.display()
        );
        std::fs::write(&config_path, config_content).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        assert!(!loaded.updates.check_enabled);
        // Should still have default interval even when disabled
        assert_eq!(loaded.updates.check_interval_hours, 24);
    }
}
