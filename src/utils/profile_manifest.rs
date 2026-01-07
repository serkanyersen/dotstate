use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Package manager types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Brew,   // Homebrew (macOS/Linux)
    Apt,    // Advanced Package Tool (Debian/Ubuntu)
    Yum,    // Yellowdog Updater Modified (RHEL/CentOS)
    Dnf,    // Dandified Yum (Fedora)
    Pacman, // Arch Linux
    Snap,   // Snap packages
    Cargo,  // Rust packages
    Npm,    // Node.js packages
    Pip,    // Python packages (pip)
    Pip3,   // Python packages (pip3)
    Gem,    // Ruby gems
    Custom, // Custom install command
}

/// Package definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    /// Display name for the package
    pub name: String,
    /// Optional description (cached metadata, not required)
    #[serde(default)]
    pub description: Option<String>,
    /// Package manager type
    pub manager: PackageManager,
    /// Package name in the manager (e.g., "eza" for brew)
    /// None for custom packages
    #[serde(default)]
    pub package_name: Option<String>,
    /// Binary name to check for existence (cached, can be derived but stored for performance)
    /// For packages with multiple binaries, this is the primary one
    pub binary_name: String,
    /// Install command (only for custom packages, derived for managed packages)
    #[serde(default)]
    pub install_command: Option<String>,
    /// Command to check if package exists (optional for custom packages, derived for managed packages)
    /// If None or empty for custom packages, the system will perform a standard existence check
    /// derived from the binary name (checking if binary exists in PATH)
    #[serde(default)]
    pub existence_check: Option<String>,
    /// Optional manager-native check command (fallback when binary_name check fails)
    /// e.g., "brew list eza" or "dpkg -s git"
    #[serde(default)]
    pub manager_check: Option<String>,
}

/// Profile manifest stored in the repository root
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProfileManifest {
    /// List of profile names
    pub profiles: Vec<ProfileInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    /// Profile name (must match folder name)
    pub name: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Files synced for this profile (relative paths from home directory)
    #[serde(default)]
    pub synced_files: Vec<String>,
    /// Packages/dependencies for this profile
    #[serde(default)]
    pub packages: Vec<Package>,
}

impl ProfileManifest {
    /// Get the path to the manifest file in the repo
    pub fn manifest_path(repo_path: &Path) -> PathBuf {
        repo_path.join(".dotstate-profiles.toml")
    }

    /// Load the manifest from the repository
    pub fn load(repo_path: &Path) -> Result<Self> {
        let manifest_path = Self::manifest_path(repo_path);

        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("Failed to read profile manifest: {:?}", manifest_path))?;
            let mut manifest: ProfileManifest =
                toml::from_str(&content).with_context(|| "Failed to parse profile manifest")?;

            // Sort synced_files alphabetically for all profiles to ensure consistent ordering
            for profile in &mut manifest.profiles {
                profile.synced_files.sort();
            }

            Ok(manifest)
        } else {
            // Return empty manifest if file doesn't exist
            Ok(Self {
                profiles: Vec::new(),
            })
        }
    }

    /// Backfill manifest from existing profile folders in the repo
    /// This is useful for repos created before the manifest system was added
    pub fn backfill_from_repo(repo_path: &Path) -> Result<Self> {
        let mut manifest = Self {
            profiles: Vec::new(),
        };

        // Scan repo directory for profile folders
        // Profile folders are directories at the repo root that aren't .git or other system files
        if let Ok(entries) = std::fs::read_dir(repo_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Skip if not a directory
                if !path.is_dir() {
                    continue;
                }

                let name = match path.file_name().and_then(|n| n.to_str()) {
                    Some(n) => n,
                    None => continue,
                };

                // Skip system directories
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }

                // Check if this looks like a profile folder (has files in it)
                if let Ok(dir_entries) = std::fs::read_dir(&path) {
                    let has_files = dir_entries.into_iter().next().is_some();
                    if has_files {
                        // This looks like a profile folder
                        manifest.add_profile(name.to_string(), None);
                    }
                }
            }
        }

        Ok(manifest)
    }

    /// Update packages for a profile
    #[allow(dead_code)] // Reserved for future use
    pub fn update_packages(&mut self, profile_name: &str, packages: Vec<Package>) -> Result<()> {
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
            profile.packages = packages;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{}' not found in manifest",
                profile_name
            ))
        }
    }

    /// Load manifest, backfilling from repo if it doesn't exist
    pub fn load_or_backfill(repo_path: &Path) -> Result<Self> {
        let manifest_path = Self::manifest_path(repo_path);

        if manifest_path.exists() {
            Self::load(repo_path)
        } else {
            // Manifest doesn't exist, try to backfill from existing folders
            let manifest = Self::backfill_from_repo(repo_path)?;

            // Save the backfilled manifest so it's available next time
            if !manifest.profiles.is_empty() {
                manifest.save(repo_path)?;
            }

            Ok(manifest)
        }
    }

    /// Save the manifest to the repository
    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let manifest_path = Self::manifest_path(repo_path);

        let content =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize profile manifest")?;

        std::fs::write(&manifest_path, content)
            .with_context(|| format!("Failed to write profile manifest: {:?}", manifest_path))?;

        Ok(())
    }

    /// Add a profile to the manifest
    pub fn add_profile(&mut self, name: String, description: Option<String>) {
        // Check if profile already exists
        if !self.profiles.iter().any(|p| p.name == name) {
            self.profiles.push(ProfileInfo {
                name,
                description,
                synced_files: Vec::new(),
                packages: Vec::new(),
            });
        }
    }

    /// Update synced files for a profile
    pub fn update_synced_files(
        &mut self,
        profile_name: &str,
        synced_files: Vec<String>,
    ) -> Result<()> {
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
            // Sort alphabetically to ensure consistent ordering and prevent unnecessary diffs
            let mut sorted_files = synced_files;
            sorted_files.sort();
            profile.synced_files = sorted_files;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{}' not found in manifest",
                profile_name
            ))
        }
    }

    // get_synced_files method removed - access synced_files directly from ProfileInfo

    /// Remove a profile from the manifest
    pub fn remove_profile(&mut self, name: &str) -> bool {
        let initial_len = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() < initial_len
    }

    /// Update a profile's name (for rename)
    pub fn rename_profile(&mut self, old_name: &str, new_name: &str) -> Result<()> {
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == old_name) {
            profile.name = new_name.to_string();
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{}' not found in manifest",
                old_name
            ))
        }
    }

    /// Get all profile names
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.iter().map(|p| p.name.clone()).collect()
    }

    /// Check if a profile exists in the manifest
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    pub fn has_profile(&self, name: &str) -> bool {
        self.profiles.iter().any(|p| p.name == name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_profile_manifest() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Create new manifest
        let mut manifest = ProfileManifest {
            profiles: Vec::new(),
        };

        // Add profiles
        manifest.add_profile("Personal".to_string(), Some("Personal Mac".to_string()));
        manifest.add_profile("Work".to_string(), None);

        // Add packages to a profile
        let packages = vec![Package {
            name: "eza".to_string(),
            description: Some("Modern replacement for ls".to_string()),
            manager: PackageManager::Brew,
            package_name: Some("eza".to_string()),
            binary_name: "eza".to_string(),
            install_command: None,
            existence_check: None,
            manager_check: None,
        }];
        manifest.update_packages("Personal", packages).unwrap();

        // Save
        manifest.save(repo_path).unwrap();

        // Load
        let mut loaded = ProfileManifest::load(repo_path).unwrap();
        assert_eq!(loaded.profiles.len(), 2);
        assert!(loaded.has_profile("Personal"));
        assert!(loaded.has_profile("Work"));

        // Rename
        loaded.rename_profile("Personal", "Personal-Mac").unwrap();
        assert!(!loaded.has_profile("Personal"));
        assert!(loaded.has_profile("Personal-Mac"));

        // Remove
        loaded.remove_profile("Work");
        assert!(!loaded.has_profile("Work"));
    }
}
