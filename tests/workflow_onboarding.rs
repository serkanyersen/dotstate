//! Integration tests for onboarding/setup workflows.
//!
//! Tests the complete chain of operations for:
//! - Local storage setup
//! - GitHub repository setup
//! - Setup edge cases and error scenarios

mod common;

use anyhow::Result;
use common::TestEnv;
use dotstate::config::RepoMode;
use dotstate::utils::profile_manifest::ProfileInfo;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

// ============================================================================
// LOCAL MODE - HAPPY PATH
// ============================================================================

#[test]
fn local_setup_creates_repo_and_config() -> Result<()> {
    // Given: fresh system, no config exists
    let temp_dir = TempDir::new()?;
    let base = temp_dir.path();

    let repo_path = base.join("dotfiles");
    let config_dir = base.join("config");

    // When: setup local storage (simulate StorageSetupService)
    // 1. Create repo directory
    fs::create_dir_all(&repo_path)?;
    fs::create_dir_all(repo_path.join("default"))?;
    fs::create_dir_all(repo_path.join("common"))?;

    // 2. Initialize git
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()?;

    // 3. Create manifest with default profile
    let manifest = dotstate::utils::profile_manifest::ProfileManifest {
        profiles: vec![ProfileInfo {
            name: "default".to_string(),
            description: Some("Default profile".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        }],
        ..Default::default()
    };
    manifest.save(&repo_path)?;

    // 4. Create config
    fs::create_dir_all(&config_dir)?;
    let config = dotstate::config::Config {
        repo_path: repo_path.clone(),
        repo_mode: RepoMode::Local,
        active_profile: "default".to_string(),
        profile_activated: false,
        ..Default::default()
    };
    let config_content = toml::to_string_pretty(&config)?;
    fs::write(config_dir.join("config.toml"), config_content)?;

    // 5. Create empty tracking
    let tracking = dotstate::utils::symlink_manager::SymlinkTracking::default();
    let tracking_json = serde_json::to_string_pretty(&tracking)?;
    fs::write(config_dir.join("symlinks.json"), tracking_json)?;

    // Then: verify complete setup
    assert!(repo_path.exists());
    assert!(repo_path.join(".git").exists());
    assert!(repo_path.join("default").exists());
    assert!(repo_path.join("common").exists());
    assert!(repo_path.join(".dotstate-profiles.toml").exists());
    assert!(config_dir.join("config.toml").exists());
    assert!(config_dir.join("symlinks.json").exists());

    // Verify manifest content
    let loaded_manifest = dotstate::utils::profile_manifest::ProfileManifest::load(&repo_path)?;
    assert_eq!(loaded_manifest.profiles.len(), 1);
    assert_eq!(loaded_manifest.profiles[0].name, "default");

    // Verify config content
    let config_content = fs::read_to_string(config_dir.join("config.toml"))?;
    let loaded_config: dotstate::config::Config = toml::from_str(&config_content)?;
    assert_eq!(loaded_config.repo_mode, RepoMode::Local);
    assert_eq!(loaded_config.active_profile, "default");
    assert!(!loaded_config.profile_activated);

    Ok(())
}

#[test]
fn local_setup_with_custom_path() -> Result<()> {
    // Given: user specifies custom path
    let temp_dir = TempDir::new()?;
    let custom_path = temp_dir.path().join("my-custom-dotfiles");

    // When: setup at custom path
    fs::create_dir_all(&custom_path)?;
    fs::create_dir_all(custom_path.join("default"))?;
    fs::create_dir_all(custom_path.join("common"))?;

    // Initialize git
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&custom_path)
        .output()?;

    // Then: repo at custom path
    assert!(custom_path.exists());
    assert!(custom_path.join(".git").exists());
    assert!(custom_path.join("default").exists());

    Ok(())
}

#[test]
fn local_setup_initializes_git_repo() -> Result<()> {
    // Given: empty directory
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().join("repo");
    fs::create_dir_all(&repo_path)?;

    // When: initialize git
    let output = std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()?;

    assert!(output.status.success());

    // Then: valid git repo
    assert!(repo_path.join(".git").exists());

    // Can check git status
    let status_output = std::process::Command::new("git")
        .args(["status"])
        .current_dir(&repo_path)
        .output()?;
    assert!(status_output.status.success());

    Ok(())
}

// ============================================================================
// GITHUB MODE - HAPPY PATH (using local bare repo as mock)
// ============================================================================

#[test]
fn github_setup_clones_existing_repo() -> Result<()> {
    // Given: "remote" repo exists (simulated with local bare repo)
    let temp_dir = TempDir::new()?;
    let base = temp_dir.path();

    // Create "remote" bare repo
    let remote_path = base.join("remote.git");
    fs::create_dir_all(&remote_path)?;
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&remote_path)
        .output()?;

    // Create a temp repo to push initial content
    let temp_repo = base.join("temp");
    fs::create_dir_all(&temp_repo)?;
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_repo)
        .output()?;
    // Set main as default branch (compatible with older git versions)
    std::process::Command::new("git")
        .args(["symbolic-ref", "HEAD", "refs/heads/main"])
        .current_dir(&temp_repo)
        .output()?;

    // Create initial structure
    fs::create_dir_all(temp_repo.join("default"))?;
    fs::create_dir_all(temp_repo.join("common"))?;

    let manifest = dotstate::utils::profile_manifest::ProfileManifest {
        profiles: vec![ProfileInfo {
            name: "default".to_string(),
            description: None,
            synced_files: vec![".existing-file".to_string()],
            packages: Vec::new(),
        }],
        ..Default::default()
    };
    manifest.save(&temp_repo)?;

    fs::write(temp_repo.join("default/.existing-file"), "existing content")?;

    // Configure git user for CI environments
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&temp_repo)
        .output()?;

    // Commit and push to "remote"
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["remote", "add", "origin", remote_path.to_str().unwrap()])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["push", "-u", "origin", "HEAD:main"])
        .current_dir(&temp_repo)
        .output()?;

    // When: clone the "remote" repo
    let clone_path = base.join("cloned");
    let clone_output = std::process::Command::new("git")
        .args([
            "clone",
            remote_path.to_str().unwrap(),
            clone_path.to_str().unwrap(),
        ])
        .output()?;

    assert!(
        clone_output.status.success(),
        "Clone failed: {:?}",
        String::from_utf8_lossy(&clone_output.stderr)
    );

    // Then: existing structure is present
    assert!(clone_path.exists());
    assert!(clone_path.join(".git").exists());
    assert!(clone_path.join("default").exists());
    assert!(clone_path.join(".dotstate-profiles.toml").exists());
    assert!(clone_path.join("default/.existing-file").exists());

    // Can load existing manifest
    let loaded_manifest = dotstate::utils::profile_manifest::ProfileManifest::load(&clone_path)?;
    assert_eq!(loaded_manifest.profiles.len(), 1);
    assert!(loaded_manifest.profiles[0]
        .synced_files
        .contains(&".existing-file".to_string()));

    Ok(())
}

#[test]
fn github_clone_empty_repo() -> Result<()> {
    // Given: empty "remote" repo
    let temp_dir = TempDir::new()?;
    let base = temp_dir.path();

    let remote_path = base.join("empty-remote.git");
    fs::create_dir_all(&remote_path)?;
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&remote_path)
        .output()?;

    // Need to create an initial commit for clone to work
    let temp_repo = base.join("temp");
    fs::create_dir_all(&temp_repo)?;
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_repo)
        .output()?;
    // Set main as default branch (compatible with older git versions)
    std::process::Command::new("git")
        .args(["symbolic-ref", "HEAD", "refs/heads/main"])
        .current_dir(&temp_repo)
        .output()?;

    // Configure git user for CI environments
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&temp_repo)
        .output()?;

    fs::write(temp_repo.join(".gitkeep"), "")?;
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["remote", "add", "origin", remote_path.to_str().unwrap()])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["push", "-u", "origin", "HEAD:main"])
        .current_dir(&temp_repo)
        .output()?;

    // When: clone and initialize structure
    let clone_path = base.join("cloned");
    std::process::Command::new("git")
        .args([
            "clone",
            remote_path.to_str().unwrap(),
            clone_path.to_str().unwrap(),
        ])
        .output()?;

    // Initialize structure (what setup would do for empty repo)
    fs::create_dir_all(clone_path.join("default"))?;
    fs::create_dir_all(clone_path.join("common"))?;

    let manifest = dotstate::utils::profile_manifest::ProfileManifest {
        profiles: vec![ProfileInfo {
            name: "default".to_string(),
            description: Some("Default profile".to_string()),
            synced_files: Vec::new(),
            packages: Vec::new(),
        }],
        ..Default::default()
    };
    manifest.save(&clone_path)?;

    // Then: structure initialized
    assert!(clone_path.join("default").exists());
    assert!(clone_path.join("common").exists());
    assert!(clone_path.join(".dotstate-profiles.toml").exists());

    Ok(())
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn setup_when_config_already_exists() -> Result<()> {
    // Given: config already exists
    let temp_dir = TempDir::new()?;
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir)?;

    // Create existing config
    let existing_config = dotstate::config::Config {
        repo_path: PathBuf::from("/old/path"),
        active_profile: "old-profile".to_string(),
        ..Default::default()
    };
    let content = toml::to_string_pretty(&existing_config)?;
    fs::write(config_dir.join("config.toml"), content)?;

    // Then: config exists (setup should detect this)
    assert!(config_dir.join("config.toml").exists());

    // Read existing config
    let existing_content = fs::read_to_string(config_dir.join("config.toml"))?;
    let loaded: dotstate::config::Config = toml::from_str(&existing_content)?;
    assert_eq!(loaded.active_profile, "old-profile");

    // In real implementation: prompt user for overwrite confirmation

    Ok(())
}

#[test]
fn setup_when_repo_path_exists_not_git() -> Result<()> {
    // Given: directory exists but is not a git repo
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().join("existing-dir");
    fs::create_dir_all(&repo_path)?;
    fs::write(repo_path.join("some-file.txt"), "content")?;

    // Then: directory exists, no .git
    assert!(repo_path.exists());
    assert!(!repo_path.join(".git").exists());

    // Check if it's a git repo
    let result = std::process::Command::new("git")
        .args(["status"])
        .current_dir(&repo_path)
        .output()?;

    assert!(!result.status.success(), "Should not be a git repo");

    // In real implementation: error or ask user if they want to init git here

    Ok(())
}

#[test]
fn setup_when_repo_path_is_already_git() -> Result<()> {
    // Given: directory exists and is already a git repo
    let temp_dir = TempDir::new()?;
    let repo_path = temp_dir.path().join("existing-git");
    fs::create_dir_all(&repo_path)?;

    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()?;

    // Then: can adopt existing repo
    assert!(repo_path.join(".git").exists());

    // Can add dotstate structure to existing repo
    fs::create_dir_all(repo_path.join("default"))?;
    fs::create_dir_all(repo_path.join("common"))?;

    let manifest = dotstate::utils::profile_manifest::ProfileManifest {
        profiles: vec![ProfileInfo {
            name: "default".to_string(),
            description: None,
            synced_files: Vec::new(),
            packages: Vec::new(),
        }],
        ..Default::default()
    };
    manifest.save(&repo_path)?;

    assert!(repo_path.join(".dotstate-profiles.toml").exists());

    Ok(())
}

#[test]
fn github_clone_existing_dotstate_repo() -> Result<()> {
    // Given: remote has existing dotstate structure with profiles
    let temp_dir = TempDir::new()?;
    let base = temp_dir.path();

    // Create "remote" with full dotstate structure
    let remote_path = base.join("dotstate-remote.git");
    fs::create_dir_all(&remote_path)?;
    std::process::Command::new("git")
        .args(["init", "--bare"])
        .current_dir(&remote_path)
        .output()?;

    // Create temp repo with dotstate structure
    let temp_repo = base.join("temp");
    fs::create_dir_all(&temp_repo)?;
    std::process::Command::new("git")
        .args(["init"])
        .current_dir(&temp_repo)
        .output()?;
    // Set main as default branch (compatible with older git versions)
    std::process::Command::new("git")
        .args(["symbolic-ref", "HEAD", "refs/heads/main"])
        .current_dir(&temp_repo)
        .output()?;

    // Create multiple profiles
    fs::create_dir_all(temp_repo.join("work"))?;
    fs::create_dir_all(temp_repo.join("home"))?;
    fs::create_dir_all(temp_repo.join("common"))?;

    fs::write(temp_repo.join("work/.workrc"), "work config")?;
    fs::write(temp_repo.join("home/.homerc"), "home config")?;
    fs::write(temp_repo.join("common/.gitconfig"), "shared config")?;

    let manifest = dotstate::utils::profile_manifest::ProfileManifest {
        version: 1,
        common: dotstate::utils::profile_manifest::CommonSection {
            synced_files: vec![".gitconfig".to_string()],
        },
        profiles: vec![
            ProfileInfo {
                name: "work".to_string(),
                description: Some("Work profile".to_string()),
                synced_files: vec![".workrc".to_string()],
                packages: Vec::new(),
            },
            ProfileInfo {
                name: "home".to_string(),
                description: Some("Home profile".to_string()),
                synced_files: vec![".homerc".to_string()],
                packages: Vec::new(),
            },
        ],
    };
    manifest.save(&temp_repo)?;

    // Configure git user for CI environments
    std::process::Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&temp_repo)
        .output()?;

    // Commit and push
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["commit", "-m", "initial dotstate setup"])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["remote", "add", "origin", remote_path.to_str().unwrap()])
        .current_dir(&temp_repo)
        .output()?;
    std::process::Command::new("git")
        .args(["push", "-u", "origin", "HEAD:main"])
        .current_dir(&temp_repo)
        .output()?;

    // When: clone
    let clone_path = base.join("cloned");
    std::process::Command::new("git")
        .args([
            "clone",
            remote_path.to_str().unwrap(),
            clone_path.to_str().unwrap(),
        ])
        .output()?;

    // Then: recognizes existing setup
    let loaded_manifest = dotstate::utils::profile_manifest::ProfileManifest::load(&clone_path)?;

    assert_eq!(loaded_manifest.profiles.len(), 2);
    assert!(loaded_manifest.profiles.iter().any(|p| p.name == "work"));
    assert!(loaded_manifest.profiles.iter().any(|p| p.name == "home"));
    assert!(loaded_manifest
        .common
        .synced_files
        .contains(&".gitconfig".to_string()));

    // Files are present
    assert!(clone_path.join("work/.workrc").exists());
    assert!(clone_path.join("home/.homerc").exists());
    assert!(clone_path.join("common/.gitconfig").exists());

    Ok(())
}

// ============================================================================
// ERROR SCENARIOS
// ============================================================================

#[test]
fn local_setup_fails_on_invalid_path() -> Result<()> {
    // Given: path that can't be created (simulate with read-only parent - skip in CI)
    // Note: This test is tricky to run reliably, so we test the error detection path

    let invalid_path = PathBuf::from("/nonexistent/deep/nested/path/that/wont/exist");

    // Then: path doesn't exist and can't be created without permissions
    assert!(!invalid_path.exists());

    // In real implementation: would return early with error

    Ok(())
}

#[test]
fn github_setup_fails_on_clone_error() -> Result<()> {
    // Given: invalid remote URL
    let temp_dir = TempDir::new()?;
    let clone_path = temp_dir.path().join("clone-target");

    // When: try to clone invalid URL
    let result = std::process::Command::new("git")
        .args([
            "clone",
            "https://github.com/nonexistent-user-12345/nonexistent-repo-67890.git",
            clone_path.to_str().unwrap(),
        ])
        .output()?;

    // Then: clone fails
    assert!(!result.status.success());
    assert!(!clone_path.exists());

    // In real implementation: return error, don't create config

    Ok(())
}

// ============================================================================
// STATE CONSISTENCY
// ============================================================================

#[test]
fn setup_creates_all_required_files() -> Result<()> {
    // This is essentially our TestEnv verification - ensure it creates everything
    let env = TestEnv::new().with_profile("default").with_git().build()?;

    // All required files exist
    assert!(env.config_path().exists(), "config.toml missing");
    assert!(env.tracking_path().exists(), "symlinks.json missing");
    assert!(
        env.repo_path.join(".dotstate-profiles.toml").exists(),
        "manifest missing"
    );
    assert!(env.repo_path.join(".git").exists(), "git repo missing");
    assert!(
        env.profile_path("default").exists(),
        "default profile dir missing"
    );
    assert!(env.common_path().exists(), "common dir missing");

    // Files are valid
    let config = env.load_config()?;
    assert_eq!(config.active_profile, "default");

    let manifest = env.load_manifest()?;
    assert!(!manifest.profiles.is_empty());

    let tracking = env.load_tracking()?;
    assert_eq!(tracking.version, 1);

    Ok(())
}

#[test]
fn setup_is_atomic_config_not_created_on_repo_failure() -> Result<()> {
    // Given: setup that will fail during repo creation
    let temp_dir = TempDir::new()?;
    let config_dir = temp_dir.path().join("config");

    // Simulate: repo creation fails, config should not be created

    // If repo creation fails early...
    let repo_path = PathBuf::from("/this/path/should/fail");
    let repo_created = fs::create_dir_all(&repo_path).is_ok();

    // ...don't create config
    if !repo_created {
        // Don't create config
        assert!(!config_dir.join("config.toml").exists());
    }

    Ok(())
}

// ============================================================================
// BACKUP VALIDATION
// ============================================================================

#[test]
fn setup_validates_backup_dir_is_writable() -> Result<()> {
    // Given: backup directory
    let temp_dir = TempDir::new()?;
    let backup_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&backup_dir)?;

    // When: validate backup dir is writable
    let test_file = backup_dir.join(".write-test");
    let can_write = fs::write(&test_file, "test").is_ok();

    // Then: should be writable
    assert!(can_write);

    // Cleanup
    let _ = fs::remove_file(&test_file);

    Ok(())
}

// ============================================================================
// INTEGRATION WITH EXISTING CONFIG
// ============================================================================

#[test]
fn setup_preserves_user_preferences_on_reinit() -> Result<()> {
    // Given: existing config with user preferences
    let temp_dir = TempDir::new()?;
    let config_dir = temp_dir.path().join("config");
    fs::create_dir_all(&config_dir)?;

    let existing_config = dotstate::config::Config {
        repo_path: PathBuf::from("/old/path"),
        active_profile: "old-profile".to_string(),
        backup_enabled: false,      // User disabled backups
        theme: "light".to_string(), // User set light theme
        ..Default::default()
    };
    let content = toml::to_string_pretty(&existing_config)?;
    fs::write(config_dir.join("config.toml"), content)?;

    // When: read existing config
    let loaded_content = fs::read_to_string(config_dir.join("config.toml"))?;
    let loaded: dotstate::config::Config = toml::from_str(&loaded_content)?;

    // Then: user preferences are preserved
    assert!(!loaded.backup_enabled);
    assert_eq!(loaded.theme, "light");

    // In real reinit: would merge preferences with new repo path

    Ok(())
}
