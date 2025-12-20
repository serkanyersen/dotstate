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
    /// Default dotfiles to scan
    pub default_dotfiles: Vec<String>,
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
    /// Profile name (e.g., "main", "work", "personal", "mac", "linux")
    pub name: String,
    /// Description of the profile
    pub description: Option<String>,
}

fn default_repo_name() -> String {
    "dotstate-storage".to_string()
}

fn default_branch_name() -> String {
    "main".to_string()
}

impl Config {
    /// Load configuration from file or create default
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

            Ok(config)
        } else {
            let config = Self::default();
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
            }],
            default_dotfiles: Self::default_dotfile_list(),
            repo_path: dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".dotstate"),
            repo_name: "dotstate-storage".to_string(),
            default_branch: "main".to_string(),
            synced_files: Vec::new(),
        }
    }

    /// Get list of default dotfiles to scan
    fn default_dotfile_list() -> Vec<String> {
        vec![
            // Shell configs
            ".bashrc".to_string(),
            ".bash_profile".to_string(),
            ".zshrc".to_string(),
            ".zprofile".to_string(),
            ".zshenv".to_string(),
            // Terminal customizations
            ".p10k.zsh".to_string(),
            ".oh-my-zsh".to_string(),
            // Editor configs
            ".vimrc".to_string(),
            ".config/nvim".to_string(),
            ".config/nvim/init.vim".to_string(),
            ".config/nvim/init.lua".to_string(),
            // Git
            ".gitconfig".to_string(),
            ".gitignore_global".to_string(),
            // Terminal
            ".tmux.conf".to_string(),
            ".config/alacritty".to_string(),
            ".config/kitty".to_string(),
            // SSH
            ".ssh/config".to_string(),
            // Fish shell
            ".config/fish".to_string(),
            // Other common configs
            ".config/wezterm".to_string(),
            ".config/starship.toml".to_string(),
        ]
    }

    /// Get the active profile
    #[allow(dead_code)]
    pub fn get_active_profile(&self) -> Option<&Profile> {
        self.profiles
            .iter()
            .find(|p| p.name == self.active_profile)
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
        assert!(!config.default_dotfiles.is_empty());
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


