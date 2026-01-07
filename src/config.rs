use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure
/// Note: Profiles are stored in the repository manifest (.dotstate-profiles.toml), not in this config file.
/// This config only stores local settings like backup preferences and active profile name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// GitHub repository information
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

    /// Get default configuration
    pub fn default() -> Self {
        Self {
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
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");
        let repo_path = temp_dir.path().join("repo");

        // Create a config with a non-existent repo_path to avoid manifest loading
        let mut config = Config::default();
        config.repo_path = repo_path.clone();
        config.save(&config_path).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        // Both should have empty active_profile since repo_path doesn't exist
        assert_eq!(config.active_profile, loaded.active_profile);
        assert_eq!(loaded.active_profile, "");
    }
}
