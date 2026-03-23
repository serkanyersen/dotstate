use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Current version of the manifest file format.
/// Increment this when making breaking changes to the schema.
const CURRENT_VERSION: u32 = 2;

/// Maximum inheritance chain depth to prevent runaway resolution.
const MAX_INHERITANCE_DEPTH: usize = 32;

/// A resolved file entry indicating where a file comes from after
/// walking the inheritance chain and merging with common files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedFile {
    /// Relative path from home directory (e.g. ".zshrc")
    pub relative_path: String,
    /// Which profile or "common" this file is sourced from.
    /// This determines the repo subdirectory: `<repo>/<source_profile>/<relative_path>`
    pub source_profile: String,
}

/// Package manager types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    /// Optional manager-native check command (fallback when `binary_name` check fails)
    /// e.g., "brew list eza" or "dpkg -s git"
    #[serde(default)]
    pub manager_check: Option<String>,
}

/// Common section for files shared across all profiles
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CommonSection {
    /// Files synced to all profiles (relative paths from home directory)
    #[serde(default)]
    pub synced_files: Vec<String>,
}

/// Reserved profile names that cannot be used
pub const RESERVED_PROFILE_NAMES: &[&str] = &["common"];

/// Profile manifest stored in the repository root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileManifest {
    /// Schema version for migration support. Missing = v0.
    #[serde(default)]
    pub version: u32,
    /// Common files shared across all profiles
    #[serde(default)]
    pub common: CommonSection,
    /// List of profile names
    #[serde(default)]
    pub profiles: Vec<ProfileInfo>,
}

impl Default for ProfileManifest {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            common: CommonSection::default(),
            profiles: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    /// Profile name (must match folder name)
    pub name: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Optional parent profile name for single inheritance.
    /// When set, activating this profile also includes files (and packages)
    /// from the parent chain. The child's files take priority over the parent's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inherits: Option<String>,
    /// Files synced for this profile (relative paths from home directory)
    #[serde(default)]
    pub synced_files: Vec<String>,
    /// Packages/dependencies for this profile
    #[serde(default)]
    pub packages: Vec<Package>,
}

impl ProfileManifest {
    /// Get the path to the manifest file in the repo
    #[must_use]
    pub fn manifest_path(repo_path: &Path) -> PathBuf {
        repo_path.join(".dotstate-profiles.toml")
    }

    /// Load the manifest from the repository.
    /// Automatically migrates old manifest versions to the current version.
    pub fn load(repo_path: &Path) -> Result<Self> {
        let manifest_path = Self::manifest_path(repo_path);

        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("Failed to read profile manifest: {manifest_path:?}"))?;
            let mut manifest: ProfileManifest =
                toml::from_str(&content).with_context(|| "Failed to parse profile manifest")?;

            // Migrate if needed
            if manifest.version < CURRENT_VERSION {
                let old_version = manifest.version;
                tracing::info!(
                    "Migrating manifest from v{} to v{}",
                    old_version,
                    CURRENT_VERSION
                );
                manifest = Self::migrate(manifest)?;

                // Backup, save, cleanup
                super::migrate_file(&manifest_path, old_version, "toml", || {
                    manifest.save(repo_path)
                })?;
            }

            // Sort synced_files alphabetically to ensure consistent ordering
            manifest.common.synced_files.sort();
            for profile in &mut manifest.profiles {
                profile.synced_files.sort();
            }

            Ok(manifest)
        } else {
            // Return empty manifest if file doesn't exist
            Ok(Self::default())
        }
    }

    /// Backfill manifest from existing profile folders in the repo
    /// This is useful for repos created before the manifest system was added
    pub fn backfill_from_repo(repo_path: &Path) -> Result<Self> {
        let mut manifest = Self::default();

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

                // Check if this looks like a profile/common folder (has files in it)
                if let Ok(dir_entries) = std::fs::read_dir(&path) {
                    let has_files = dir_entries.into_iter().next().is_some();
                    if has_files {
                        if name == "common" {
                            // This is the common folder - backfill common files
                            if let Ok(common_files) = Self::scan_folder_files(&path) {
                                manifest.common.synced_files = common_files;
                            }
                        } else {
                            // This looks like a profile folder
                            manifest.add_profile(name.to_string(), None);
                        }
                    }
                }
            }
        }

        Ok(manifest)
    }

    /// Scan a folder for files (used during backfill)
    fn scan_folder_files(folder_path: &Path) -> Result<Vec<String>> {
        let mut files = Vec::new();
        Self::scan_folder_files_recursive(folder_path, folder_path, &mut files)?;
        files.sort();
        Ok(files)
    }

    /// Recursively scan folder for files
    fn scan_folder_files_recursive(
        base_path: &Path,
        current_path: &Path,
        files: &mut Vec<String>,
    ) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(current_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                let relative = path
                    .strip_prefix(base_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();

                if path.is_dir() {
                    // Recurse into subdirectories
                    Self::scan_folder_files_recursive(base_path, &path, files)?;
                } else {
                    files.push(relative);
                }
            }
        }
        Ok(())
    }

    /// Update packages for a profile
    #[allow(dead_code)] // Reserved for future use
    pub fn update_packages(&mut self, profile_name: &str, packages: Vec<Package>) -> Result<()> {
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
            profile.packages = packages;
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{profile_name}' not found in manifest"
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

    /// Save the manifest to the repository.
    /// Uses atomic write (temp file + rename) to prevent corruption on crash.
    pub fn save(&self, repo_path: &Path) -> Result<()> {
        let manifest_path = Self::manifest_path(repo_path);
        let temp_path = manifest_path.with_extension("toml.tmp");

        let content =
            toml::to_string_pretty(self).with_context(|| "Failed to serialize profile manifest")?;

        // Write to temp file first
        std::fs::write(&temp_path, &content)
            .with_context(|| format!("Failed to write temp manifest: {temp_path:?}"))?;

        // Atomic rename (on POSIX systems)
        std::fs::rename(&temp_path, &manifest_path)
            .with_context(|| format!("Failed to rename temp manifest to {manifest_path:?}"))?;

        Ok(())
    }

    /// Add a profile to the manifest
    pub fn add_profile(&mut self, name: String, description: Option<String>) {
        self.add_profile_with_inherits(name, description, None);
    }

    /// Add a profile to the manifest with optional inheritance
    pub fn add_profile_with_inherits(
        &mut self,
        name: String,
        description: Option<String>,
        inherits: Option<String>,
    ) {
        // Check if profile already exists
        if !self.profiles.iter().any(|p| p.name == name) {
            self.profiles.push(ProfileInfo {
                name,
                description,
                inherits,
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
                "Profile '{profile_name}' not found in manifest"
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
        let mut found = false;
        for profile in &mut self.profiles {
            if profile.name == old_name {
                profile.name = new_name.to_string();
                found = true;
            }
            // Update any inherits references pointing to the old name
            if profile.inherits.as_deref() == Some(old_name) {
                profile.inherits = Some(new_name.to_string());
            }
        }
        if found {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{old_name}' not found in manifest"
            ))
        }
    }

    /// Get all profile names
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    #[must_use]
    pub fn profile_names(&self) -> Vec<String> {
        self.profiles.iter().map(|p| p.name.clone()).collect()
    }

    /// Check if a profile exists in the manifest
    #[allow(dead_code)] // Kept for potential future use in CLI or programmatic access
    #[must_use]
    pub fn has_profile(&self, name: &str) -> bool {
        self.profiles.iter().any(|p| p.name == name)
    }

    /// Check if a name is reserved and cannot be used as a profile name
    #[must_use]
    pub fn is_reserved_name(name: &str) -> bool {
        RESERVED_PROFILE_NAMES.contains(&name.to_lowercase().as_str())
    }

    /// Add a file to the common section
    pub fn add_common_file(&mut self, relative_path: &str) {
        let path = relative_path.to_string();
        if !self.common.synced_files.contains(&path) {
            self.common.synced_files.push(path);
            self.common.synced_files.sort();
        }
    }

    /// Remove a file from the common section
    pub fn remove_common_file(&mut self, relative_path: &str) -> bool {
        let initial_len = self.common.synced_files.len();
        self.common.synced_files.retain(|f| f != relative_path);
        self.common.synced_files.len() < initial_len
    }

    /// Get all common files
    #[must_use]
    pub fn get_common_files(&self) -> &[String] {
        &self.common.synced_files
    }

    /// Check if a file is in the common section
    #[must_use]
    pub fn is_common_file(&self, relative_path: &str) -> bool {
        self.common
            .synced_files
            .contains(&relative_path.to_string())
    }

    // ==================== Migration Methods ====================

    /// Run all necessary migrations to bring manifest to current version.
    fn migrate(mut manifest: Self) -> Result<Self> {
        if manifest.version == 0 {
            manifest = Self::migrate_v0_to_v1(manifest)?;
        }
        if manifest.version == 1 {
            manifest = Self::migrate_v1_to_v2(manifest)?;
        }
        Ok(manifest)
    }

    /// Migrate from v0 (no version field) to v1.
    /// This is a no-op migration that just sets the version field.
    fn migrate_v0_to_v1(mut manifest: Self) -> Result<Self> {
        tracing::debug!("Migrating manifest v0 -> v1");
        manifest.version = 1;
        Ok(manifest)
    }

    /// Migrate from v1 to v2 (adds `inherits` field to profiles).
    /// This is a no-op migration since `inherits` defaults to `None` via serde.
    fn migrate_v1_to_v2(mut manifest: Self) -> Result<Self> {
        tracing::debug!("Migrating manifest v1 -> v2 (adds profile inheritance support)");
        manifest.version = 2;
        Ok(manifest)
    }

    // ==================== Inheritance Methods ====================

    /// Build the inheritance chain for a profile, from child to root ancestor.
    ///
    /// Returns a list of profile names starting with `profile_name` and ending
    /// with the root ancestor (the profile that has no `inherits`).
    ///
    /// # Errors
    /// - If a profile in the chain is not found in the manifest.
    /// - If a cycle is detected.
    /// - If the chain exceeds `MAX_INHERITANCE_DEPTH`.
    pub fn inheritance_chain(&self, profile_name: &str) -> Result<Vec<String>> {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut current = profile_name.to_string();

        loop {
            if visited.contains(&current) {
                return Err(anyhow::anyhow!(
                    "Inheritance cycle detected: '{}' appears twice in chain: [{}]",
                    current,
                    chain.join(" -> ")
                ));
            }
            if chain.len() >= MAX_INHERITANCE_DEPTH {
                return Err(anyhow::anyhow!(
                    "Inheritance chain too deep (max {MAX_INHERITANCE_DEPTH}): [{}]",
                    chain.join(" -> ")
                ));
            }

            visited.insert(current.clone());
            chain.push(current.clone());

            let profile = self
                .profiles
                .iter()
                .find(|p| p.name == current)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Profile '{}' not found in manifest (referenced in inheritance chain: [{}])",
                        current,
                        chain.join(" -> ")
                    )
                })?;

            match &profile.inherits {
                Some(parent) => {
                    current = parent.clone();
                }
                None => break,
            }
        }

        Ok(chain)
    }

    /// Resolve the full list of files for a profile, walking the inheritance
    /// chain and merging with common files.
    ///
    /// Resolution priority (highest to lowest):
    /// 1. Active profile's own `synced_files`
    /// 2. Parent profile's `synced_files`
    /// 3. Grandparent, etc.
    /// 4. Common files (lowest priority, overridden by any profile in chain)
    ///
    /// Each file appears at most once; the highest-priority source wins.
    pub fn resolve_files(&self, profile_name: &str) -> Result<Vec<ResolvedFile>> {
        let chain = self.inheritance_chain(profile_name)?;

        // Build a map: relative_path -> source_profile
        // Walk from root ancestor to child so that child overrides parent
        let mut file_map: HashMap<String, String> = HashMap::new();

        // First pass: walk from root ancestor to child (reverse of chain)
        for profile_name in chain.iter().rev() {
            if let Some(profile) = self.profiles.iter().find(|p| &p.name == profile_name) {
                for file in &profile.synced_files {
                    file_map.insert(file.clone(), profile_name.clone());
                }
            }
        }

        // Second pass: add common files only where no profile in chain provides them
        for file in &self.common.synced_files {
            file_map
                .entry(file.clone())
                .or_insert_with(|| "common".to_string());
        }

        // Convert to sorted Vec<ResolvedFile>
        let mut resolved: Vec<ResolvedFile> = file_map
            .into_iter()
            .map(|(relative_path, source_profile)| ResolvedFile {
                relative_path,
                source_profile,
            })
            .collect();
        resolved.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));

        Ok(resolved)
    }

    /// Resolve the full list of packages for a profile, walking the inheritance
    /// chain and merging.
    ///
    /// Child packages override parent packages with the same `name + manager` key.
    pub fn resolve_packages(&self, profile_name: &str) -> Result<Vec<Package>> {
        let chain = self.inheritance_chain(profile_name)?;

        // Walk from root ancestor to child so child overrides parent
        // Key: (package_name, manager) -> Package
        let mut pkg_map: HashMap<(String, PackageManager), Package> = HashMap::new();

        for profile_name in chain.iter().rev() {
            if let Some(profile) = self.profiles.iter().find(|p| &p.name == profile_name) {
                for pkg in &profile.packages {
                    let key = (pkg.name.clone(), pkg.manager.clone());
                    pkg_map.insert(key, pkg.clone());
                }
            }
        }

        let mut packages: Vec<Package> = pkg_map.into_values().collect();
        packages.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(packages)
    }

    /// Validate the inheritance configuration of the entire manifest.
    ///
    /// Checks:
    /// - All `inherits` targets exist in the manifest.
    /// - No cycles exist.
    /// - No chain exceeds `MAX_INHERITANCE_DEPTH`.
    pub fn validate_inheritance(&self) -> Result<()> {
        for profile in &self.profiles {
            if let Some(parent_name) = &profile.inherits {
                // Check parent exists
                if !self.profiles.iter().any(|p| p.name == *parent_name) {
                    return Err(anyhow::anyhow!(
                        "Profile '{}' inherits from '{}', which does not exist",
                        profile.name,
                        parent_name
                    ));
                }

                // Validate the full chain (checks for cycles and depth)
                self.inheritance_chain(&profile.name)?;
            }
        }
        Ok(())
    }

    /// Get profiles that directly inherit from the given profile.
    #[must_use]
    pub fn get_inheriting_profiles(&self, profile_name: &str) -> Vec<String> {
        self.profiles
            .iter()
            .filter(|p| p.inherits.as_deref() == Some(profile_name))
            .map(|p| p.name.clone())
            .collect()
    }

    /// Set the `inherits` field for a profile.
    pub fn set_inherits(&mut self, profile_name: &str, inherits: Option<String>) -> Result<()> {
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
            let old_inherits = profile.inherits.clone();
            profile.inherits = inherits;
            // Validate the whole manifest to catch cycles
            if let Err(e) = self.validate_inheritance() {
                // Revert
                if let Some(p) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
                    p.inherits = old_inherits;
                }
                return Err(e);
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{profile_name}' not found in manifest"
            ))
        }
    }

    /// Move a file from a profile to common
    pub fn move_to_common(&mut self, profile_name: &str, relative_path: &str) -> Result<()> {
        // Remove from profile
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
            profile.synced_files.retain(|f| f != relative_path);
        } else {
            return Err(anyhow::anyhow!(
                "Profile '{profile_name}' not found in manifest"
            ));
        }

        // Add to common
        self.add_common_file(relative_path);
        Ok(())
    }

    /// Move a file from common to a profile
    pub fn move_from_common(&mut self, profile_name: &str, relative_path: &str) -> Result<()> {
        // Remove from common
        if !self.remove_common_file(relative_path) {
            return Err(anyhow::anyhow!(
                "File '{relative_path}' not found in common section"
            ));
        }

        // Add to profile
        if let Some(profile) = self.profiles.iter_mut().find(|p| p.name == profile_name) {
            if !profile.synced_files.contains(&relative_path.to_string()) {
                profile.synced_files.push(relative_path.to_string());
                profile.synced_files.sort();
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Profile '{profile_name}' not found in manifest"
            ))
        }
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
        let mut manifest = ProfileManifest::default();

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

    #[test]
    fn test_reserved_names() {
        assert!(ProfileManifest::is_reserved_name("common"));
        assert!(ProfileManifest::is_reserved_name("Common"));
        assert!(ProfileManifest::is_reserved_name("COMMON"));
        assert!(!ProfileManifest::is_reserved_name("work"));
        assert!(!ProfileManifest::is_reserved_name("personal"));
    }

    #[test]
    fn test_common_files() {
        let mut manifest = ProfileManifest::default();

        // Add common files
        manifest.add_common_file(".gitconfig");
        manifest.add_common_file(".tmux.conf");
        assert_eq!(manifest.get_common_files().len(), 2);
        assert!(manifest.is_common_file(".gitconfig"));
        assert!(manifest.is_common_file(".tmux.conf"));

        // Adding duplicate should not increase count
        manifest.add_common_file(".gitconfig");
        assert_eq!(manifest.get_common_files().len(), 2);

        // Remove common file
        assert!(manifest.remove_common_file(".tmux.conf"));
        assert_eq!(manifest.get_common_files().len(), 1);
        assert!(!manifest.is_common_file(".tmux.conf"));

        // Remove non-existent should return false
        assert!(!manifest.remove_common_file(".nonexistent"));
    }

    #[test]
    fn test_move_to_common() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("work".to_string(), None);

        // Add file to profile
        manifest
            .update_synced_files("work", vec![".zshrc".to_string()])
            .unwrap();

        // Move to common
        manifest.move_to_common("work", ".zshrc").unwrap();

        // Verify file is in common and not in profile
        assert!(manifest.is_common_file(".zshrc"));
        let profile = manifest.profiles.iter().find(|p| p.name == "work").unwrap();
        assert!(!profile.synced_files.contains(&".zshrc".to_string()));
    }

    #[test]
    fn test_move_from_common() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("work".to_string(), None);

        // Add file to common
        manifest.add_common_file(".gitconfig");

        // Move to profile
        manifest.move_from_common("work", ".gitconfig").unwrap();

        // Verify file is in profile and not in common
        assert!(!manifest.is_common_file(".gitconfig"));
        let profile = manifest.profiles.iter().find(|p| p.name == "work").unwrap();
        assert!(profile.synced_files.contains(&".gitconfig".to_string()));
    }

    #[test]
    fn test_manifest_migration_v0_to_v1() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Write a v0 manifest (no version field)
        let v0_manifest = r#"
[common]
synced_files = [".gitconfig"]

[[profiles]]
name = "work"
synced_files = [".zshrc"]
"#;
        std::fs::write(ProfileManifest::manifest_path(repo_path), v0_manifest).unwrap();

        // Load should auto-migrate to current version (v0 -> v1 -> v2)
        let loaded = ProfileManifest::load(repo_path).unwrap();
        assert_eq!(loaded.version, CURRENT_VERSION);
        assert!(loaded.is_common_file(".gitconfig"));
        assert!(loaded.has_profile("work"));

        // File should be updated with current version
        let content = std::fs::read_to_string(ProfileManifest::manifest_path(repo_path)).unwrap();
        assert!(content.contains(&format!("version = {CURRENT_VERSION}")));

        // Backup should be cleaned up
        let backup_path =
            ProfileManifest::manifest_path(repo_path).with_extension("toml.backup-v0");
        assert!(!backup_path.exists());
    }

    #[test]
    fn test_manifest_already_at_current_version() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Write a manifest at the current version
        let current_manifest = format!(
            r#"
version = {CURRENT_VERSION}

[common]
synced_files = []

[[profiles]]
name = "test"
synced_files = []
"#
        );
        std::fs::write(ProfileManifest::manifest_path(repo_path), current_manifest).unwrap();

        // Load should not create backup (no migration needed)
        let loaded = ProfileManifest::load(repo_path).unwrap();
        assert_eq!(loaded.version, CURRENT_VERSION);

        // No backup should exist
        let backup_path =
            ProfileManifest::manifest_path(repo_path).with_extension("toml.backup-v0");
        assert!(!backup_path.exists());
    }

    #[test]
    fn test_new_manifest_has_current_version() {
        let manifest = ProfileManifest::default();
        assert_eq!(manifest.version, CURRENT_VERSION);
    }

    #[test]
    fn test_manifest_migration_v1_to_v2() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // Write a v1 manifest (no inherits field)
        let v1_manifest = r#"
version = 1

[common]
synced_files = [".gitconfig"]

[[profiles]]
name = "work"
synced_files = [".zshrc"]
"#;
        std::fs::write(ProfileManifest::manifest_path(repo_path), v1_manifest).unwrap();

        // Load should auto-migrate to v2
        let loaded = ProfileManifest::load(repo_path).unwrap();
        assert_eq!(loaded.version, 2);
        assert!(loaded.is_common_file(".gitconfig"));
        assert!(loaded.has_profile("work"));
        // inherits should default to None
        let work = loaded.profiles.iter().find(|p| p.name == "work").unwrap();
        assert!(work.inherits.is_none());
    }

    #[test]
    fn test_inherits_field_serialization() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        let mut manifest = ProfileManifest::default();
        manifest.add_profile("base".to_string(), None);
        manifest.add_profile_with_inherits(
            "child".to_string(),
            Some("Child profile".to_string()),
            Some("base".to_string()),
        );
        manifest.save(repo_path).unwrap();

        // Reload and verify
        let loaded = ProfileManifest::load(repo_path).unwrap();
        let base = loaded.profiles.iter().find(|p| p.name == "base").unwrap();
        assert!(base.inherits.is_none());

        let child = loaded.profiles.iter().find(|p| p.name == "child").unwrap();
        assert_eq!(child.inherits, Some("base".to_string()));
    }

    #[test]
    fn test_inheritance_chain_simple() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("grandparent".to_string(), None);
        manifest.add_profile_with_inherits(
            "parent".to_string(),
            None,
            Some("grandparent".to_string()),
        );
        manifest.add_profile_with_inherits("child".to_string(), None, Some("parent".to_string()));

        let chain = manifest.inheritance_chain("child").unwrap();
        assert_eq!(chain, vec!["child", "parent", "grandparent"]);
    }

    #[test]
    fn test_inheritance_chain_no_parent() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("standalone".to_string(), None);

        let chain = manifest.inheritance_chain("standalone").unwrap();
        assert_eq!(chain, vec!["standalone"]);
    }

    #[test]
    fn test_inheritance_cycle_detection() {
        let mut manifest = ProfileManifest::default();
        manifest.profiles.push(ProfileInfo {
            name: "a".to_string(),
            description: None,
            inherits: Some("b".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        });
        manifest.profiles.push(ProfileInfo {
            name: "b".to_string(),
            description: None,
            inherits: Some("a".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        });

        let result = manifest.inheritance_chain("a");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cycle"));
    }

    #[test]
    fn test_inheritance_missing_parent() {
        let mut manifest = ProfileManifest::default();
        manifest.profiles.push(ProfileInfo {
            name: "orphan".to_string(),
            description: None,
            inherits: Some("nonexistent".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        });

        let result = manifest.inheritance_chain("orphan");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_files_with_inheritance() {
        let mut manifest = ProfileManifest::default();
        manifest.common.synced_files = vec![".gitconfig".to_string(), ".tmux.conf".to_string()];

        manifest.profiles.push(ProfileInfo {
            name: "p1".to_string(),
            description: None,
            inherits: None,
            synced_files: vec![".zshrc".to_string(), ".vimrc".to_string()],
            packages: Vec::new(),
        });
        manifest.profiles.push(ProfileInfo {
            name: "p2".to_string(),
            description: None,
            inherits: Some("p1".to_string()),
            synced_files: vec![".vimrc".to_string(), ".config/nvim".to_string()],
            packages: Vec::new(),
        });

        let resolved = manifest.resolve_files("p2").unwrap();

        // Expected:
        // .config/nvim -> p2 (own)
        // .gitconfig -> common (not overridden)
        // .tmux.conf -> common (not overridden)
        // .vimrc -> p2 (child wins over p1)
        // .zshrc -> p1 (inherited)
        assert_eq!(resolved.len(), 5);

        let find = |path: &str| resolved.iter().find(|r| r.relative_path == path).unwrap();
        assert_eq!(find(".config/nvim").source_profile, "p2");
        assert_eq!(find(".gitconfig").source_profile, "common");
        assert_eq!(find(".tmux.conf").source_profile, "common");
        assert_eq!(find(".vimrc").source_profile, "p2"); // child wins
        assert_eq!(find(".zshrc").source_profile, "p1"); // inherited
    }

    #[test]
    fn test_resolve_files_profile_overrides_common() {
        let mut manifest = ProfileManifest::default();
        manifest.common.synced_files = vec![".gitconfig".to_string()];

        manifest.profiles.push(ProfileInfo {
            name: "p1".to_string(),
            description: None,
            inherits: None,
            synced_files: vec![".gitconfig".to_string()], // same as common
            packages: Vec::new(),
        });

        let resolved = manifest.resolve_files("p1").unwrap();
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].source_profile, "p1"); // profile wins over common
    }

    #[test]
    fn test_resolve_files_no_inheritance() {
        let mut manifest = ProfileManifest::default();
        manifest.common.synced_files = vec![".gitconfig".to_string()];

        manifest.profiles.push(ProfileInfo {
            name: "standalone".to_string(),
            description: None,
            inherits: None,
            synced_files: vec![".zshrc".to_string()],
            packages: Vec::new(),
        });

        let resolved = manifest.resolve_files("standalone").unwrap();
        assert_eq!(resolved.len(), 2);

        let find = |path: &str| resolved.iter().find(|r| r.relative_path == path).unwrap();
        assert_eq!(find(".gitconfig").source_profile, "common");
        assert_eq!(find(".zshrc").source_profile, "standalone");
    }

    #[test]
    fn test_resolve_packages_with_inheritance() {
        let mut manifest = ProfileManifest::default();

        let eza_pkg = Package {
            name: "eza".to_string(),
            description: Some("ls replacement".to_string()),
            manager: PackageManager::Brew,
            package_name: Some("eza".to_string()),
            binary_name: "eza".to_string(),
            install_command: None,
            existence_check: None,
            manager_check: None,
        };
        let bat_pkg = Package {
            name: "bat".to_string(),
            description: Some("cat replacement".to_string()),
            manager: PackageManager::Brew,
            package_name: Some("bat".to_string()),
            binary_name: "bat".to_string(),
            install_command: None,
            existence_check: None,
            manager_check: None,
        };
        let fzf_pkg = Package {
            name: "fzf".to_string(),
            description: Some("fuzzy finder".to_string()),
            manager: PackageManager::Brew,
            package_name: Some("fzf".to_string()),
            binary_name: "fzf".to_string(),
            install_command: None,
            existence_check: None,
            manager_check: None,
        };

        manifest.profiles.push(ProfileInfo {
            name: "p1".to_string(),
            description: None,
            inherits: None,
            synced_files: Vec::new(),
            packages: vec![eza_pkg.clone(), bat_pkg],
        });
        manifest.profiles.push(ProfileInfo {
            name: "p2".to_string(),
            description: None,
            inherits: Some("p1".to_string()),
            synced_files: Vec::new(),
            packages: vec![fzf_pkg],
        });

        let packages = manifest.resolve_packages("p2").unwrap();
        assert_eq!(packages.len(), 3);

        let names: Vec<&str> = packages.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"eza"));
        assert!(names.contains(&"bat"));
        assert!(names.contains(&"fzf"));
    }

    #[test]
    fn test_validate_inheritance_valid() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("base".to_string(), None);
        manifest.add_profile_with_inherits("child".to_string(), None, Some("base".to_string()));

        assert!(manifest.validate_inheritance().is_ok());
    }

    #[test]
    fn test_validate_inheritance_missing_parent() {
        let mut manifest = ProfileManifest::default();
        manifest.profiles.push(ProfileInfo {
            name: "orphan".to_string(),
            description: None,
            inherits: Some("ghost".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        });

        assert!(manifest.validate_inheritance().is_err());
    }

    #[test]
    fn test_validate_inheritance_cycle() {
        let mut manifest = ProfileManifest::default();
        manifest.profiles.push(ProfileInfo {
            name: "a".to_string(),
            description: None,
            inherits: Some("b".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        });
        manifest.profiles.push(ProfileInfo {
            name: "b".to_string(),
            description: None,
            inherits: Some("a".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        });

        assert!(manifest.validate_inheritance().is_err());
    }

    #[test]
    fn test_set_inherits() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("base".to_string(), None);
        manifest.add_profile("child".to_string(), None);

        // Set inheritance
        manifest
            .set_inherits("child", Some("base".to_string()))
            .unwrap();
        assert_eq!(
            manifest
                .profiles
                .iter()
                .find(|p| p.name == "child")
                .unwrap()
                .inherits,
            Some("base".to_string())
        );

        // Clear inheritance
        manifest.set_inherits("child", None).unwrap();
        assert!(manifest
            .profiles
            .iter()
            .find(|p| p.name == "child")
            .unwrap()
            .inherits
            .is_none());
    }

    #[test]
    fn test_set_inherits_cycle_prevention() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile_with_inherits("a".to_string(), None, Some("b".to_string()));
        manifest.add_profile("b".to_string(), None);

        // Try to create a cycle: b -> a (a already -> b)
        let result = manifest.set_inherits("b", Some("a".to_string()));
        assert!(result.is_err());
        // b should remain without inherits (reverted)
        assert!(manifest
            .profiles
            .iter()
            .find(|p| p.name == "b")
            .unwrap()
            .inherits
            .is_none());
    }

    #[test]
    fn test_get_inheriting_profiles() {
        let mut manifest = ProfileManifest::default();
        manifest.add_profile("base".to_string(), None);
        manifest.add_profile_with_inherits("child1".to_string(), None, Some("base".to_string()));
        manifest.add_profile_with_inherits("child2".to_string(), None, Some("base".to_string()));
        manifest.add_profile("standalone".to_string(), None);

        let children = manifest.get_inheriting_profiles("base");
        assert_eq!(children.len(), 2);
        assert!(children.contains(&"child1".to_string()));
        assert!(children.contains(&"child2".to_string()));

        assert!(manifest.get_inheriting_profiles("standalone").is_empty());
    }

    #[test]
    fn test_three_level_inheritance() {
        let mut manifest = ProfileManifest::default();
        manifest.common.synced_files = vec![".gitconfig".to_string()];

        manifest.profiles.push(ProfileInfo {
            name: "grandparent".to_string(),
            description: None,
            inherits: None,
            synced_files: vec![".zshrc".to_string(), ".bashrc".to_string()],
            packages: Vec::new(),
        });
        manifest.profiles.push(ProfileInfo {
            name: "parent".to_string(),
            description: None,
            inherits: Some("grandparent".to_string()),
            synced_files: vec![".zshrc".to_string(), ".vimrc".to_string()], // overrides grandparent .zshrc
            packages: Vec::new(),
        });
        manifest.profiles.push(ProfileInfo {
            name: "child".to_string(),
            description: None,
            inherits: Some("parent".to_string()),
            synced_files: vec![".config/nvim".to_string()], // adds new file only
            packages: Vec::new(),
        });

        let resolved = manifest.resolve_files("child").unwrap();
        assert_eq!(resolved.len(), 5);

        let find = |path: &str| resolved.iter().find(|r| r.relative_path == path).unwrap();
        assert_eq!(find(".bashrc").source_profile, "grandparent"); // inherited through
        assert_eq!(find(".config/nvim").source_profile, "child"); // own
        assert_eq!(find(".gitconfig").source_profile, "common"); // common
        assert_eq!(find(".vimrc").source_profile, "parent"); // from parent
        assert_eq!(find(".zshrc").source_profile, "parent"); // parent overrode grandparent
    }
}
