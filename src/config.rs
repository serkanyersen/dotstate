use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// GitHub repository information
    pub github: Option<GitHubConfig>,
    /// Current active profile/set
    pub active_profile: String,
    /// Available profiles/sets
    pub profiles: Vec<Profile>,
    /// Repository root path (where dotfiles are stored locally)
    pub repo_path: PathBuf,
    /// Repository name on GitHub (default: dotstate-storage)
    #[serde(default = "default_repo_name")]
    pub repo_name: String,
    /// Default branch name (default: main)
    #[serde(default = "default_branch_name")]
    pub default_branch: String,
    /// List of synced files (relative paths from home directory)
    #[serde(default)]
    pub synced_files: Vec<String>,
    /// Whether to create backups before syncing (default: true)
    #[serde(default = "default_backup_enabled")]
    pub backup_enabled: bool,
    /// Whether the active profile is currently activated (symlinks created)
    #[serde(default)]
    pub profile_activated: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Profile name (e.g., "Personal-Mac", "Work-Linux")
    pub name: String,
    /// Description of the profile
    #[serde(default)]
    pub description: Option<String>,
    /// Files synced for this profile
    #[serde(default)]
    pub synced_files: Vec<String>,
}

impl Profile {
    /// Create a new profile
    pub fn new(name: String, description: Option<String>) -> Self {
        Self {
            name,
            description,
            synced_files: Vec::new(),
        }
    }

    /// Add a file to this profile's synced files
    pub fn add_file(&mut self, file: String) {
        if !self.synced_files.contains(&file) {
            self.synced_files.push(file);
        }
    }

    /// Remove a file from this profile's synced files
    pub fn remove_file(&mut self, file: &str) {
        self.synced_files.retain(|f| f != file);
    }

    /// Check if a file is synced in this profile
    pub fn has_file(&self, file: &str) -> bool {
        self.synced_files.contains(&file.to_string())
    }

    /// Get the profile folder path in the repository
    pub fn get_profile_path(&self, repo_path: &Path) -> PathBuf {
        repo_path.join(&self.name)
    }
}

fn default_repo_name() -> String {
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
            let content = std::fs::read_to_string(config_path)
                .with_context(|| format!("Failed to read config file: {:?}", config_path))?;
            let mut config: Config = toml::from_str(&content)
                .with_context(|| "Failed to parse config file")?;

            // Set defaults for missing fields (for backward compatibility)
            if config.repo_name.is_empty() {
                config.repo_name = default_repo_name();
            }
            if config.default_branch.is_empty() {
                config.default_branch = default_branch_name();
            }
            // backup_enabled defaults to true if not present
            // (handled by serde default)

            Ok(config)
        } else {
            // Config doesn't exist - try to discover profiles from repo
            let mut config = Self::default();

            // Try to discover profiles from the repo manifest if repo_path exists
            if config.repo_path.exists() {
                // Use load_or_backfill to handle repos created before manifest system
                if let Ok(manifest) = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path) {
                    // Convert manifest profiles to config profiles
                    config.profiles = manifest.profiles.into_iter().map(|info| {
                        Profile {
                            name: info.name,
                            description: info.description,
                            synced_files: Vec::new(), // Will be populated when user syncs files
                        }
                    }).collect();

                    // Set active profile to first one if available
                    if !config.profiles.is_empty() {
                        config.active_profile = config.profiles[0].name.clone();
                    }
                }
            }

            config.save(config_path)?;
            Ok(config)
        }
    }

    /// Save configuration to file with secure permissions
    pub fn save(&self, config_path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .with_context(|| "Failed to serialize config")?;

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
            active_profile: "Personal".to_string(),
            profiles: vec![Profile {
                name: "Personal".to_string(),
                description: Some("Default profile".to_string()),
                synced_files: Vec::new(),
            }],
            backup_enabled: true,
            profile_activated: false,
            repo_path: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".dotstate"),
            repo_name: "dotstate-storage".to_string(),
            default_branch: "main".to_string(),
            synced_files: Vec::new(),
        }
    }


    /// Get the active profile
    #[allow(dead_code)]
    pub fn get_active_profile(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.name == self.active_profile)
    }

    /// Get a mutable reference to the active profile
    pub fn get_active_profile_mut(&mut self) -> Option<&mut Profile> {
        let active_name = self.active_profile.clone();
        self.profiles
            .iter_mut()
            .find(|p| p.name == active_name)
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.name == name)
    }

    /// Get a mutable reference to a profile by name
    pub fn get_profile_mut(&mut self, name: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.name == name)
    }

    /// Add a new profile
    pub fn add_profile(&mut self, profile: Profile) {
        self.profiles.push(profile);
    }

    /// Remove a profile by name
    pub fn remove_profile(&mut self, name: &str) -> bool {
        let len_before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() < len_before
    }

    /// Check if a profile name exists
    pub fn has_profile(&self, name: &str) -> bool {
        self.profiles.iter().any(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.active_profile, "Personal");
    }

    #[test]
    fn test_config_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.toml");

        let config = Config::default();
        config.save(&config_path).unwrap();

        let loaded = Config::load_or_create(&config_path).unwrap();
        assert_eq!(config.active_profile, loaded.active_profile);
    }
}


