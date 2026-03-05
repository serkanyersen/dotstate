//! Shared test utilities for critical workflow integration tests.
//!
//! Provides `TestEnv` - a complete isolated DotState environment for testing,
//! with automatic cleanup via `TempDir`.
//!
//! ## E2E Tests with Real Services
//!
//! To test with actual service calls (SyncService, ProfileService, etc.),
//! use `.with_env_override()` on the builder. This sets environment variables
//! that redirect DotState's path functions to the test directories.
//!
//! **Important**: Tests using env overrides should run serially to avoid
//! conflicts. Run with: `cargo test --test workflow_sync -- --test-threads=1`

use anyhow::{Context, Result};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::TempDir;

use dotstate::config::{Config, RepoMode};
use dotstate::utils::profile_manifest::{ProfileInfo, ProfileManifest};
use dotstate::utils::symlink_manager::{SymlinkTracking, TrackedSymlink};

/// Global mutex to ensure only one test uses env overrides at a time.
/// This prevents race conditions when multiple tests try to set env vars.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Guard that restores environment variables when dropped.
struct EnvGuard {
    old_home: Option<String>,
    old_config: Option<String>,
    old_backup: Option<String>,
    #[allow(dead_code)]
    lock: std::sync::MutexGuard<'static, ()>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // Restore original env vars
        match &self.old_home {
            Some(v) => std::env::set_var("DOTSTATE_TEST_HOME", v),
            None => std::env::remove_var("DOTSTATE_TEST_HOME"),
        }
        match &self.old_config {
            Some(v) => std::env::set_var("DOTSTATE_TEST_CONFIG_DIR", v),
            None => std::env::remove_var("DOTSTATE_TEST_CONFIG_DIR"),
        }
        match &self.old_backup {
            Some(v) => std::env::set_var("DOTSTATE_TEST_BACKUP_DIR", v),
            None => std::env::remove_var("DOTSTATE_TEST_BACKUP_DIR"),
        }
    }
}

/// A complete isolated DotState test environment.
///
/// Creates a temporary directory structure mimicking a real DotState setup:
/// - `home/` - fake home directory for dotfiles
/// - `repo/` - storage repository
/// - `config/` - config directory (~/.config/dotstate equivalent)
/// - `backups/` - backup directory
///
/// Automatically cleaned up when dropped. If env overrides are enabled,
/// the original environment is restored on drop.
#[allow(dead_code)]
pub struct TestEnv {
    temp_dir: TempDir,
    pub home_dir: PathBuf,
    pub repo_path: PathBuf,
    pub config_dir: PathBuf,
    pub backup_dir: PathBuf,
    env_guard: Option<EnvGuard>,
}

#[allow(dead_code)]
impl TestEnv {
    /// Create a new TestEnvBuilder for fluent configuration.
    pub fn new() -> TestEnvBuilder {
        TestEnvBuilder::default()
    }

    // ==================== Path Helpers ====================

    /// Get absolute path for a file relative to the fake home directory.
    pub fn home_path(&self, relative: &str) -> PathBuf {
        self.home_dir.join(relative)
    }

    /// Get absolute path for a file relative to the repository.
    pub fn repo_file_path(&self, relative: &str) -> PathBuf {
        self.repo_path.join(relative)
    }

    /// Get the path for a profile's directory in the repo.
    /// Note: Profiles are stored directly at repo_path/<profile_name>/, not repo_path/profiles/<profile_name>/
    pub fn profile_path(&self, profile_name: &str) -> PathBuf {
        self.repo_path.join(profile_name)
    }

    /// Get the path for a file within a profile.
    pub fn profile_file_path(&self, profile_name: &str, file_relative: &str) -> PathBuf {
        self.profile_path(profile_name).join(file_relative)
    }

    /// Get the path to the common files directory.
    pub fn common_path(&self) -> PathBuf {
        self.repo_path.join("common")
    }

    /// Get the config file path.
    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    /// Get the symlink tracking file path.
    pub fn tracking_path(&self) -> PathBuf {
        self.config_dir.join("symlinks.json")
    }

    // ==================== State Inspection ====================

    /// Load the current config from disk.
    pub fn load_config(&self) -> Result<Config> {
        let content = fs::read_to_string(self.config_path()).context("Failed to read config")?;
        toml::from_str(&content).context("Failed to parse config")
    }

    /// Save the config to disk.
    pub fn save_config(&self, config: &Config) -> Result<()> {
        let content = toml::to_string_pretty(config)?;
        fs::write(self.config_path(), content)?;
        Ok(())
    }

    /// Load the current manifest from disk.
    pub fn load_manifest(&self) -> Result<ProfileManifest> {
        ProfileManifest::load(&self.repo_path)
    }

    /// Load the current symlink tracking from disk.
    pub fn load_tracking(&self) -> Result<SymlinkTracking> {
        let tracking_path = self.tracking_path();
        if tracking_path.exists() {
            let content = fs::read_to_string(&tracking_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            Ok(SymlinkTracking::default())
        }
    }

    /// Save the symlink tracking to disk.
    pub fn save_tracking(&self, tracking: &SymlinkTracking) -> Result<()> {
        let content = serde_json::to_string_pretty(tracking)?;
        fs::write(self.tracking_path(), content)?;
        Ok(())
    }

    /// Get the target of a symlink in the home directory.
    /// Returns None if path doesn't exist or isn't a symlink.
    pub fn symlink_target(&self, home_relative: &str) -> Option<PathBuf> {
        let path = self.home_path(home_relative);
        fs::read_link(&path).ok()
    }

    /// Read file content at given path.
    pub fn file_content(&self, path: &Path) -> Option<String> {
        fs::read_to_string(path).ok()
    }

    /// Read file content relative to home.
    pub fn home_file_content(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.home_path(relative)).ok()
    }

    /// Check if a path exists in the home directory.
    pub fn home_file_exists(&self, relative: &str) -> bool {
        self.home_path(relative).exists()
    }

    /// Check if a path is a symlink.
    pub fn is_symlink(&self, path: &Path) -> bool {
        path.symlink_metadata()
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    }

    // ==================== Assertions ====================

    /// Assert that a symlink exists in home and points to the expected repo location.
    pub fn assert_symlink_points_to(&self, home_relative: &str, expected_target: &Path) {
        let home_path = self.home_path(home_relative);
        assert!(
            self.is_symlink(&home_path),
            "Expected {} to be a symlink, but it isn't",
            home_path.display()
        );
        let actual_target = fs::read_link(&home_path)
            .unwrap_or_else(|e| panic!("Failed to read symlink {}: {}", home_path.display(), e));
        assert_eq!(
            actual_target,
            expected_target,
            "Symlink {} points to {:?}, expected {:?}",
            home_path.display(),
            actual_target,
            expected_target
        );
    }

    /// Assert that a path in home is a symlink (target doesn't matter).
    pub fn assert_is_symlink(&self, home_relative: &str) {
        let home_path = self.home_path(home_relative);
        assert!(
            self.is_symlink(&home_path),
            "Expected {} to be a symlink, but it isn't (exists: {})",
            home_path.display(),
            home_path.exists()
        );
    }

    /// Assert that a file in home is NOT a symlink (or doesn't exist).
    pub fn assert_no_symlink(&self, home_relative: &str) {
        let home_path = self.home_path(home_relative);
        assert!(
            !self.is_symlink(&home_path),
            "Expected {} to NOT be a symlink, but it is",
            home_path.display()
        );
    }

    /// Assert that a file exists and is a regular file (not symlink).
    pub fn assert_regular_file(&self, path: &Path) {
        assert!(path.exists(), "Expected {} to exist", path.display());
        assert!(
            !self.is_symlink(path),
            "Expected {} to be a regular file, not a symlink",
            path.display()
        );
    }

    /// Assert that a file exists in home and is a regular file (not symlink).
    pub fn assert_home_regular_file(&self, relative: &str) {
        self.assert_regular_file(&self.home_path(relative));
    }

    /// Assert that a file is tracked in symlinks.json.
    pub fn assert_file_tracked(&self, home_relative: &str) {
        let tracking = self.load_tracking().expect("Failed to load tracking");
        let home_path = self.home_path(home_relative);
        assert!(
            tracking.symlinks.iter().any(|s| s.target == home_path),
            "Expected {} to be tracked, but it isn't. Tracked targets: {:?}",
            home_relative,
            tracking
                .symlinks
                .iter()
                .map(|s| &s.target)
                .collect::<Vec<_>>()
        );
    }

    /// Assert that a file is NOT tracked in symlinks.json.
    pub fn assert_file_not_tracked(&self, home_relative: &str) {
        let tracking = self.load_tracking().expect("Failed to load tracking");
        let home_path = self.home_path(home_relative);
        assert!(
            !tracking.symlinks.iter().any(|s| s.target == home_path),
            "Expected {} to NOT be tracked, but it is",
            home_relative
        );
    }

    /// Assert that a file is listed in a profile's synced_files in the manifest.
    pub fn assert_file_in_profile(&self, profile_name: &str, file_relative: &str) {
        let manifest = self.load_manifest().expect("Failed to load manifest");
        let profile = manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .unwrap_or_else(|| panic!("Profile {} not found in manifest", profile_name));
        assert!(
            profile.synced_files.contains(&file_relative.to_string()),
            "Expected {} to be in profile {}'s synced_files, but it isn't. Files: {:?}",
            file_relative,
            profile_name,
            profile.synced_files
        );
    }

    /// Assert that a file is NOT in a profile's synced_files.
    pub fn assert_file_not_in_profile(&self, profile_name: &str, file_relative: &str) {
        let manifest = self.load_manifest().expect("Failed to load manifest");
        let profile = manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .unwrap_or_else(|| panic!("Profile {} not found in manifest", profile_name));
        assert!(
            !profile.synced_files.contains(&file_relative.to_string()),
            "Expected {} to NOT be in profile {}'s synced_files, but it is",
            file_relative,
            profile_name
        );
    }

    /// Assert that a file is in the common section of the manifest.
    pub fn assert_file_in_common(&self, file_relative: &str) {
        let manifest = self.load_manifest().expect("Failed to load manifest");
        assert!(
            manifest
                .common
                .synced_files
                .contains(&file_relative.to_string()),
            "Expected {} to be in common synced_files, but it isn't. Files: {:?}",
            file_relative,
            manifest.common.synced_files
        );
    }

    /// Assert that a file is NOT in the common section.
    pub fn assert_file_not_in_common(&self, file_relative: &str) {
        let manifest = self.load_manifest().expect("Failed to load manifest");
        assert!(
            !manifest
                .common
                .synced_files
                .contains(&file_relative.to_string()),
            "Expected {} to NOT be in common synced_files, but it is",
            file_relative
        );
    }

    /// Assert that a backup exists for a file (checks backup directory has files).
    pub fn assert_backup_exists_for(&self, _original_relative: &str) {
        let backup_count = fs::read_dir(&self.backup_dir)
            .map(|entries| entries.count())
            .unwrap_or(0);
        assert!(
            backup_count > 0,
            "Expected backup to exist in {:?}, but backup dir is empty",
            self.backup_dir
        );
    }

    /// Assert that a profile exists in the manifest.
    pub fn assert_profile_exists(&self, profile_name: &str) {
        let manifest = self.load_manifest().expect("Failed to load manifest");
        assert!(
            manifest.profiles.iter().any(|p| p.name == profile_name),
            "Expected profile {} to exist, but it doesn't. Profiles: {:?}",
            profile_name,
            manifest
                .profiles
                .iter()
                .map(|p| &p.name)
                .collect::<Vec<_>>()
        );
    }

    /// Assert that a profile does NOT exist in the manifest.
    pub fn assert_profile_not_exists(&self, profile_name: &str) {
        let manifest = self.load_manifest().expect("Failed to load manifest");
        assert!(
            !manifest.profiles.iter().any(|p| p.name == profile_name),
            "Expected profile {} to NOT exist, but it does",
            profile_name
        );
    }

    /// Assert that a specific profile is the active one in config.
    pub fn assert_active_profile(&self, expected_profile: &str) {
        let config = self.load_config().expect("Failed to load config");
        assert_eq!(
            config.active_profile, expected_profile,
            "Expected active profile to be {}, but it's {}",
            expected_profile, config.active_profile
        );
    }

    /// Assert that profile is activated (symlinks created).
    pub fn assert_profile_activated(&self) {
        let config = self.load_config().expect("Failed to load config");
        assert!(
            config.profile_activated,
            "Expected profile to be activated, but it isn't"
        );
    }

    /// Assert that profile is NOT activated.
    pub fn assert_profile_not_activated(&self) {
        let config = self.load_config().expect("Failed to load config");
        assert!(
            !config.profile_activated,
            "Expected profile to NOT be activated, but it is"
        );
    }

    // ==================== Mutations (for simulating states) ====================

    /// Delete a symlink from home without updating tracking (simulates crash).
    pub fn delete_symlink_without_tracking(&self, home_relative: &str) -> Result<()> {
        let path = self.home_path(home_relative);
        fs::remove_file(&path).context("Failed to delete symlink")
    }

    /// Delete a file from the repo without updating manifest (simulates corruption).
    pub fn delete_repo_file_without_manifest(&self, repo_relative: &str) -> Result<()> {
        let path = self.repo_file_path(repo_relative);
        fs::remove_file(&path).context("Failed to delete repo file")
    }

    /// Add an entry to tracking without creating the actual symlink (simulates crash).
    pub fn add_tracking_without_symlink(&self, home_relative: &str, source: &Path) -> Result<()> {
        let mut tracking = self.load_tracking()?;
        tracking.symlinks.push(TrackedSymlink {
            target: self.home_path(home_relative),
            source: source.to_path_buf(),
            created_at: chrono::Utc::now(),
            backup: None,
        });
        self.save_tracking(&tracking)
    }

    /// Create a symlink without updating tracking (simulates manual intervention).
    pub fn create_symlink_without_tracking(
        &self,
        home_relative: &str,
        target: &Path,
    ) -> Result<()> {
        let link_path = self.home_path(home_relative);
        if let Some(parent) = link_path.parent() {
            fs::create_dir_all(parent)?;
        }
        symlink(target, &link_path).context("Failed to create symlink")
    }

    /// Create a regular file in home (useful for setting up conflict scenarios).
    pub fn create_home_file(&self, relative: &str, content: &str) -> Result<()> {
        let path = self.home_path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, content)?;
        Ok(())
    }

    /// Remove a file from manifest without touching the filesystem.
    pub fn remove_from_manifest(&self, profile_name: &str, file_relative: &str) -> Result<()> {
        let mut manifest = self.load_manifest()?;
        if let Some(profile) = manifest
            .profiles
            .iter_mut()
            .find(|p| p.name == profile_name)
        {
            profile.synced_files.retain(|f| f != file_relative);
        }
        manifest.save(&self.repo_path)
    }
}

/// Builder for TestEnv with fluent configuration.
#[derive(Default)]
#[allow(dead_code)]
pub struct TestEnvBuilder {
    profiles: Vec<(String, Option<String>)>, // (name, description)
    active_profile: Option<String>,
    profile_activated: bool,
    home_files: Vec<(String, String)>, // (relative_path, content)
    synced_files: Vec<(String, String, String)>, // (profile_name, relative_path, content)
    common_files: Vec<(String, String)>, // (relative_path, content)
    backup_enabled: bool,
    init_git: bool,
    env_override: bool,
}

#[allow(dead_code)]
impl TestEnvBuilder {
    /// Add a profile to the manifest.
    pub fn with_profile(mut self, name: &str) -> Self {
        self.profiles.push((name.to_string(), None));
        self
    }

    /// Add a profile with description.
    pub fn with_profile_desc(mut self, name: &str, description: &str) -> Self {
        self.profiles
            .push((name.to_string(), Some(description.to_string())));
        self
    }

    /// Set the active profile and mark as activated.
    pub fn with_activated_profile(mut self, name: &str) -> Self {
        self.active_profile = Some(name.to_string());
        self.profile_activated = true;
        self
    }

    /// Set the active profile without marking as activated (profile selected but symlinks not created).
    pub fn with_selected_profile(mut self, name: &str) -> Self {
        self.active_profile = Some(name.to_string());
        self.profile_activated = false;
        self
    }

    /// Add a file to the fake home directory (not synced).
    pub fn with_home_file(mut self, relative_path: &str, content: &str) -> Self {
        self.home_files
            .push((relative_path.to_string(), content.to_string()));
        self
    }

    /// Add a file that is already synced (in repo, symlinked from home, tracked).
    pub fn with_synced_file(mut self, profile: &str, relative_path: &str, content: &str) -> Self {
        self.synced_files.push((
            profile.to_string(),
            relative_path.to_string(),
            content.to_string(),
        ));
        self
    }

    /// Add a common file (synced across all profiles).
    pub fn with_common_file(mut self, relative_path: &str, content: &str) -> Self {
        self.common_files
            .push((relative_path.to_string(), content.to_string()));
        self
    }

    /// Enable backups.
    pub fn with_backup_enabled(mut self) -> Self {
        self.backup_enabled = true;
        self
    }

    /// Initialize git repository.
    pub fn with_git(mut self) -> Self {
        self.init_git = true;
        self
    }

    /// Enable environment variable overrides for E2E testing.
    ///
    /// When enabled, sets `DOTSTATE_TEST_HOME`, `DOTSTATE_TEST_CONFIG_DIR`,
    /// and `DOTSTATE_TEST_BACKUP_DIR` to point to the test directories.
    /// This allows calling real services (SyncService, ProfileService, etc.)
    /// that use `get_home_dir()` and `get_config_dir()` internally.
    ///
    /// The environment is automatically restored when TestEnv is dropped.
    ///
    /// **Note**: Tests using this should run serially (`--test-threads=1`)
    /// to avoid env var conflicts between parallel tests.
    pub fn with_env_override(mut self) -> Self {
        self.env_override = true;
        self
    }

    /// Build the test environment.
    pub fn build(self) -> Result<TestEnv> {
        let temp_dir = TempDir::new().context("Failed to create temp dir")?;
        let base = temp_dir.path();

        // Create directory structure
        let home_dir = base.join("home");
        let repo_path = base.join("repo");
        let config_dir = base.join("config");
        let backup_dir = base.join("backups");

        fs::create_dir_all(&home_dir)?;
        fs::create_dir_all(&repo_path)?;
        fs::create_dir_all(&config_dir)?;
        fs::create_dir_all(&backup_dir)?;
        fs::create_dir_all(repo_path.join("common"))?;

        // Initialize git if requested
        if self.init_git {
            std::process::Command::new("git")
                .args(["init"])
                .current_dir(&repo_path)
                .output()
                .context("Failed to init git")?;
        }

        // Create non-synced home files
        for (relative_path, content) in &self.home_files {
            let full_path = home_dir.join(relative_path);
            if let Some(parent) = full_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&full_path, content)?;
        }

        // Build manifest
        let mut manifest = ProfileManifest::default();

        // Add profiles (stored directly at repo_path/<name>/, not repo_path/profiles/<name>/)
        for (name, description) in &self.profiles {
            let profile_dir = repo_path.join(name);
            fs::create_dir_all(&profile_dir)?;

            manifest.profiles.push(ProfileInfo {
                name: name.clone(),
                description: description.clone(),
                inherits: None,
                synced_files: Vec::new(),
                packages: Vec::new(),
            });
        }

        // Build tracking
        let mut tracking = SymlinkTracking {
            version: 1,
            active_profile: self.active_profile.clone().unwrap_or_default(),
            symlinks: Vec::new(),
        };

        // Process synced files - create in repo, create symlinks if activated, update manifest
        for (profile, relative_path, content) in &self.synced_files {
            let home_path = home_dir.join(relative_path);
            let repo_file_path = repo_path.join(profile).join(relative_path);

            // Create parent dirs in repo
            if let Some(parent) = repo_file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write file to repo
            fs::write(&repo_file_path, content)?;

            // Create symlink if profile is activated and this is the active profile
            if self.profile_activated && self.active_profile.as_deref() == Some(profile) {
                if let Some(parent) = home_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                symlink(&repo_file_path, &home_path)?;

                // Add to tracking
                tracking.symlinks.push(TrackedSymlink {
                    target: home_path.clone(),
                    source: repo_file_path.clone(),
                    created_at: chrono::Utc::now(),
                    backup: None,
                });
            }

            // Update manifest
            if let Some(profile_info) = manifest.profiles.iter_mut().find(|p| &p.name == profile) {
                profile_info.synced_files.push(relative_path.clone());
            }
        }

        // Process common files
        for (relative_path, content) in &self.common_files {
            let home_path = home_dir.join(relative_path);
            let common_file_path = repo_path.join("common").join(relative_path);

            // Create parent dirs
            if let Some(parent) = common_file_path.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write file to common
            fs::write(&common_file_path, content)?;

            // Create symlink if any profile is activated
            if self.profile_activated {
                if let Some(parent) = home_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                symlink(&common_file_path, &home_path)?;

                tracking.symlinks.push(TrackedSymlink {
                    target: home_path.clone(),
                    source: common_file_path.clone(),
                    created_at: chrono::Utc::now(),
                    backup: None,
                });
            }

            manifest.common.synced_files.push(relative_path.clone());
        }

        // Save manifest
        manifest.save(&repo_path)?;

        // Save tracking
        let tracking_json = serde_json::to_string_pretty(&tracking)?;
        fs::write(config_dir.join("symlinks.json"), tracking_json)?;

        // Create config
        let config = Config {
            repo_path: repo_path.clone(),
            repo_mode: RepoMode::Local,
            active_profile: self
                .active_profile
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            profile_activated: self.profile_activated,
            backup_enabled: self.backup_enabled,
            ..Default::default()
        };

        let config_content = toml::to_string_pretty(&config)?;
        fs::write(config_dir.join("config.toml"), config_content)?;

        // Set up environment overrides if requested
        let env_guard = if self.env_override {
            // Acquire mutex to prevent parallel tests from conflicting
            // Use unwrap_or_else to recover from a poisoned mutex (from a panicked test)
            let lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

            // Save current values
            let old_home = std::env::var("DOTSTATE_TEST_HOME").ok();
            let old_config = std::env::var("DOTSTATE_TEST_CONFIG_DIR").ok();
            let old_backup = std::env::var("DOTSTATE_TEST_BACKUP_DIR").ok();

            // Set new values
            std::env::set_var("DOTSTATE_TEST_HOME", &home_dir);
            std::env::set_var("DOTSTATE_TEST_CONFIG_DIR", &config_dir);
            std::env::set_var("DOTSTATE_TEST_BACKUP_DIR", &backup_dir);

            Some(EnvGuard {
                old_home,
                old_config,
                old_backup,
                lock,
            })
        } else {
            None
        };

        Ok(TestEnv {
            temp_dir,
            home_dir,
            repo_path,
            config_dir,
            backup_dir,
            env_guard,
        })
    }
}

// ==================== Convenience Constructors ====================

/// Helper to create a minimal test environment with defaults.
#[allow(dead_code)]
pub fn minimal_env() -> Result<TestEnv> {
    TestEnv::new().with_profile("default").build()
}

/// Helper to create a test environment with an activated profile.
#[allow(dead_code)]
pub fn activated_env() -> Result<TestEnv> {
    TestEnv::new()
        .with_profile("default")
        .with_activated_profile("default")
        .build()
}

/// Helper to create a test environment with a synced file.
#[allow(dead_code)]
pub fn env_with_synced_file(filename: &str, content: &str) -> Result<TestEnv> {
    TestEnv::new()
        .with_profile("default")
        .with_activated_profile("default")
        .with_synced_file("default", filename, content)
        .build()
}
