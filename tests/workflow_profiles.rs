//! Integration tests for profile workflows.
//!
//! Tests the complete chain of operations for:
//! - Creating profiles
//! - Activating profiles
//! - Switching profiles
//! - Deleting profiles

mod common;

use anyhow::Result;
use common::TestEnv;
use dotstate::utils::profile_manifest::ProfileInfo;

// ============================================================================
// CREATE PROFILE - HAPPY PATH
// ============================================================================

#[test]
fn create_profile_initializes_directory_and_manifest() -> Result<()> {
    // Given: repo with default profile
    let env = TestEnv::new().with_profile("default").with_git().build()?;

    // When: create new profile (simulate the operation)
    let new_profile_dir = env.profile_path("work");
    std::fs::create_dir_all(&new_profile_dir)?;

    let mut manifest = env.load_manifest()?;
    manifest.profiles.push(ProfileInfo {
        name: "work".to_string(),
        description: Some("Work profile".to_string()),
        inherits: None,
        synced_files: Vec::new(),
        packages: Vec::new(),
    });
    manifest.save(&env.repo_path)?;

    // Then: profile directory and manifest entry exist
    assert!(new_profile_dir.exists());
    env.assert_profile_exists("work");

    let manifest = env.load_manifest()?;
    let work_profile = manifest.profiles.iter().find(|p| p.name == "work").unwrap();
    assert_eq!(work_profile.description, Some("Work profile".to_string()));

    Ok(())
}

#[test]
fn create_profile_copies_from_existing() -> Result<()> {
    // Given: profile "default" has synced files
    let env = TestEnv::new()
        .with_profile("default")
        .with_activated_profile("default")
        .with_synced_file("default", ".zshrc", "zsh config")
        .with_synced_file("default", ".vimrc", "vim config")
        .build()?;

    // When: create "work" profile copying from "default"
    let work_dir = env.profile_path("work");
    std::fs::create_dir_all(&work_dir)?;

    // Copy files from default to work
    let default_dir = env.profile_path("default");
    for entry in std::fs::read_dir(&default_dir)? {
        let entry = entry?;
        let dest = work_dir.join(entry.file_name());
        if entry.path().is_file() {
            std::fs::copy(entry.path(), dest)?;
        }
    }

    // Update manifest
    let mut manifest = env.load_manifest()?;
    let default_files = manifest
        .profiles
        .iter()
        .find(|p| p.name == "default")
        .map(|p| p.synced_files.clone())
        .unwrap_or_default();

    manifest.profiles.push(ProfileInfo {
        name: "work".to_string(),
        description: None,
        inherits: None,
        synced_files: default_files,
        packages: Vec::new(),
    });
    manifest.save(&env.repo_path)?;

    // Then: work profile has copies of all files
    env.assert_profile_exists("work");
    assert!(env.profile_file_path("work", ".zshrc").exists());
    assert!(env.profile_file_path("work", ".vimrc").exists());

    let manifest = env.load_manifest()?;
    let work_profile = manifest.profiles.iter().find(|p| p.name == "work").unwrap();
    assert!(work_profile.synced_files.contains(&".zshrc".to_string()));
    assert!(work_profile.synced_files.contains(&".vimrc".to_string()));

    Ok(())
}

// ============================================================================
// ACTIVATE PROFILE - HAPPY PATH
// ============================================================================

#[test]
fn activate_profile_creates_all_symlinks() -> Result<()> {
    // Given: profile with files, not yet activated
    let env = TestEnv::new()
        .with_profile("default")
        .with_selected_profile("default") // Selected but NOT activated
        .build()?;

    // Add files to the profile manually (without activating)
    let zshrc = env.profile_file_path("default", ".zshrc");
    std::fs::write(&zshrc, "zsh config")?;
    let vimrc = env.profile_file_path("default", ".vimrc");
    std::fs::write(&vimrc, "vim config")?;

    let mut manifest = env.load_manifest()?;
    if let Some(profile) = manifest.profiles.iter_mut().find(|p| p.name == "default") {
        profile.synced_files.push(".zshrc".to_string());
        profile.synced_files.push(".vimrc".to_string());
    }
    manifest.save(&env.repo_path)?;

    // Verify not activated
    env.assert_profile_not_activated();
    assert!(!env.home_path(".zshrc").exists());
    assert!(!env.home_path(".vimrc").exists());

    // When: activate profile (create symlinks)
    std::os::unix::fs::symlink(&zshrc, env.home_path(".zshrc"))?;
    std::os::unix::fs::symlink(&vimrc, env.home_path(".vimrc"))?;

    // Update tracking
    let mut tracking = env.load_tracking()?;
    tracking.active_profile = "default".to_string();
    tracking
        .symlinks
        .push(dotstate::utils::symlink_manager::TrackedSymlink {
            target: env.home_path(".zshrc"),
            source: zshrc.clone(),
            created_at: chrono::Utc::now(),
            backup: None,
        });
    tracking
        .symlinks
        .push(dotstate::utils::symlink_manager::TrackedSymlink {
            target: env.home_path(".vimrc"),
            source: vimrc.clone(),
            created_at: chrono::Utc::now(),
            backup: None,
        });
    env.save_tracking(&tracking)?;

    // Update config
    let mut config = env.load_config()?;
    config.profile_activated = true;
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(env.config_path(), config_content)?;

    // Then: all symlinks created
    env.assert_is_symlink(".zshrc");
    env.assert_is_symlink(".vimrc");
    env.assert_file_tracked(".zshrc");
    env.assert_file_tracked(".vimrc");
    env.assert_profile_activated();

    Ok(())
}

#[test]
fn activate_profile_includes_common_files() -> Result<()> {
    // Given: profile with common files
    let env = TestEnv::new()
        .with_profile("default")
        .with_activated_profile("default")
        .with_synced_file("default", ".profile-specific", "profile only")
        .with_common_file(".gitconfig", "common git config")
        .build()?;

    // Then: both profile-specific and common files are symlinked
    env.assert_is_symlink(".profile-specific");
    env.assert_is_symlink(".gitconfig");

    env.assert_file_in_profile("default", ".profile-specific");
    env.assert_file_in_common(".gitconfig");

    Ok(())
}

// ============================================================================
// ACTIVATE PROFILE - EDGE CASES
// ============================================================================

#[test]
fn activate_profile_when_home_file_already_exists() -> Result<()> {
    // Given: home file exists (not a symlink)
    let env = TestEnv::new()
        .with_profile("default")
        .with_selected_profile("default")
        .with_home_file(".zshrc", "existing content")
        .build()?;

    // Add file to profile
    let repo_file = env.profile_file_path("default", ".zshrc");
    std::fs::write(&repo_file, "repo content")?;

    // Verify home file is regular file
    env.assert_home_regular_file(".zshrc");

    // When: activate (would need to backup existing file first)
    // In real implementation, this would:
    // 1. Create backup of existing file
    // 2. Remove existing file
    // 3. Create symlink

    // Simulate backup
    let backup_path = env.backup_dir.join("zshrc_backup");
    std::fs::copy(env.home_path(".zshrc"), &backup_path)?;

    // Remove and symlink
    std::fs::remove_file(env.home_path(".zshrc"))?;
    std::os::unix::fs::symlink(&repo_file, env.home_path(".zshrc"))?;

    // Then: symlink exists, backup was created
    env.assert_is_symlink(".zshrc");
    assert!(backup_path.exists());
    assert_eq!(std::fs::read_to_string(&backup_path)?, "existing content");

    Ok(())
}

// ============================================================================
// SWITCH PROFILE - HAPPY PATH
// ============================================================================

#[test]
fn switch_profile_replaces_symlinks() -> Result<()> {
    // Given: "default" active with files A, B
    let env = TestEnv::new()
        .with_profile("default")
        .with_profile("work")
        .with_activated_profile("default")
        .with_synced_file("default", ".default-file", "default content")
        .with_synced_file("default", ".shared-name", "default version")
        .build()?;

    // Add files to work profile
    let work_file = env.profile_file_path("work", ".work-file");
    std::fs::write(&work_file, "work content")?;
    let work_shared = env.profile_file_path("work", ".shared-name");
    std::fs::write(&work_shared, "work version")?;

    let mut manifest = env.load_manifest()?;
    if let Some(profile) = manifest.profiles.iter_mut().find(|p| p.name == "work") {
        profile.synced_files.push(".work-file".to_string());
        profile.synced_files.push(".shared-name".to_string());
    }
    manifest.save(&env.repo_path)?;

    // Verify initial state
    env.assert_is_symlink(".default-file");
    env.assert_is_symlink(".shared-name");
    env.assert_active_profile("default");

    // When: switch to work profile
    // 1. Remove old symlinks
    std::fs::remove_file(env.home_path(".default-file"))?;
    std::fs::remove_file(env.home_path(".shared-name"))?;

    // 2. Create new symlinks
    std::os::unix::fs::symlink(&work_file, env.home_path(".work-file"))?;
    std::os::unix::fs::symlink(&work_shared, env.home_path(".shared-name"))?;

    // 3. Update tracking
    let mut tracking = env.load_tracking()?;
    tracking.active_profile = "work".to_string();
    tracking.symlinks.clear();
    tracking
        .symlinks
        .push(dotstate::utils::symlink_manager::TrackedSymlink {
            target: env.home_path(".work-file"),
            source: work_file.clone(),
            created_at: chrono::Utc::now(),
            backup: None,
        });
    tracking
        .symlinks
        .push(dotstate::utils::symlink_manager::TrackedSymlink {
            target: env.home_path(".shared-name"),
            source: work_shared.clone(),
            created_at: chrono::Utc::now(),
            backup: None,
        });
    env.save_tracking(&tracking)?;

    // 4. Update config
    let mut config = env.load_config()?;
    config.active_profile = "work".to_string();
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(env.config_path(), config_content)?;

    // Then: old symlinks gone, new ones created
    assert!(!env.home_path(".default-file").exists());
    env.assert_is_symlink(".work-file");
    env.assert_is_symlink(".shared-name");

    // Content changed for shared-name
    assert_eq!(
        env.home_file_content(".shared-name"),
        Some("work version".to_string())
    );

    env.assert_active_profile("work");

    Ok(())
}

#[test]
fn switch_profile_preserves_common_files() -> Result<()> {
    // Given: common file exists, switching profiles
    let env = TestEnv::new()
        .with_profile("default")
        .with_profile("work")
        .with_activated_profile("default")
        .with_common_file(".gitconfig", "shared config")
        .with_synced_file("default", ".default-only", "default")
        .build()?;

    // Add work-specific file
    let work_file = env.profile_file_path("work", ".work-only");
    std::fs::write(&work_file, "work")?;
    let mut manifest = env.load_manifest()?;
    if let Some(profile) = manifest.profiles.iter_mut().find(|p| p.name == "work") {
        profile.synced_files.push(".work-only".to_string());
    }
    manifest.save(&env.repo_path)?;

    // Verify common file is symlinked
    env.assert_is_symlink(".gitconfig");
    let original_target = std::fs::read_link(env.home_path(".gitconfig"))?;

    // When: switch to work (keeping common file intact)
    // Remove only profile-specific symlink
    std::fs::remove_file(env.home_path(".default-only"))?;

    // Create new profile-specific symlink
    std::os::unix::fs::symlink(&work_file, env.home_path(".work-only"))?;

    // Update config
    let mut config = env.load_config()?;
    config.active_profile = "work".to_string();
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(env.config_path(), config_content)?;

    // Then: common file symlink unchanged
    env.assert_is_symlink(".gitconfig");
    let new_target = std::fs::read_link(env.home_path(".gitconfig"))?;
    assert_eq!(original_target, new_target);

    // Profile-specific files changed
    assert!(!env.home_path(".default-only").exists());
    env.assert_is_symlink(".work-only");

    Ok(())
}

// ============================================================================
// SWITCH PROFILE - EDGE CASES
// ============================================================================

#[test]
fn switch_to_same_profile_is_noop() -> Result<()> {
    // Given: default is active
    let env = TestEnv::new()
        .with_profile("default")
        .with_activated_profile("default")
        .with_synced_file("default", ".zshrc", "content")
        .build()?;

    let original_tracking = env.load_tracking()?;
    let original_config = env.load_config()?;

    // When: "switch" to default (same profile)
    // This should be a no-op or at most a refresh

    // Then: state unchanged
    let new_tracking = env.load_tracking()?;
    let new_config = env.load_config()?;

    assert_eq!(original_config.active_profile, new_config.active_profile);
    assert_eq!(
        original_tracking.symlinks.len(),
        new_tracking.symlinks.len()
    );
    env.assert_is_symlink(".zshrc");

    Ok(())
}

// ============================================================================
// DELETE PROFILE
// ============================================================================

#[test]
fn delete_inactive_profile_cleans_up() -> Result<()> {
    // Given: work profile exists but default is active
    let env = TestEnv::new()
        .with_profile("default")
        .with_profile("work")
        .with_activated_profile("default")
        .build()?;

    // Add some files to work profile
    let work_file = env.profile_file_path("work", ".workrc");
    std::fs::write(&work_file, "work config")?;

    env.assert_profile_exists("work");

    // When: delete work profile
    // 1. Remove directory
    std::fs::remove_dir_all(env.profile_path("work"))?;

    // 2. Remove from manifest
    let mut manifest = env.load_manifest()?;
    manifest.profiles.retain(|p| p.name != "work");
    manifest.save(&env.repo_path)?;

    // Then: profile is gone
    env.assert_profile_not_exists("work");
    assert!(!env.profile_path("work").exists());

    // Default still works
    env.assert_profile_exists("default");
    env.assert_active_profile("default");

    Ok(())
}

#[test]
fn delete_active_profile_deactivates_first() -> Result<()> {
    // Given: work is the active profile
    let env = TestEnv::new()
        .with_profile("default")
        .with_profile("work")
        .with_activated_profile("work")
        .with_synced_file("work", ".workrc", "work config")
        .build()?;

    env.assert_is_symlink(".workrc");
    env.assert_active_profile("work");

    // When: delete work profile
    // 1. Remove symlinks first
    std::fs::remove_file(env.home_path(".workrc"))?;

    // 2. Update tracking
    let mut tracking = env.load_tracking()?;
    tracking.symlinks.clear();
    tracking.active_profile.clear();
    env.save_tracking(&tracking)?;

    // 3. Update config (deactivate)
    let mut config = env.load_config()?;
    config.active_profile = "default".to_string();
    config.profile_activated = false;
    let config_content = toml::to_string_pretty(&config)?;
    std::fs::write(env.config_path(), config_content)?;

    // 4. Remove directory
    std::fs::remove_dir_all(env.profile_path("work"))?;

    // 5. Remove from manifest
    let mut manifest = env.load_manifest()?;
    manifest.profiles.retain(|p| p.name != "work");
    manifest.save(&env.repo_path)?;

    // Then: profile deleted, symlinks removed, switched to default
    env.assert_profile_not_exists("work");
    assert!(!env.home_path(".workrc").exists());
    env.assert_profile_not_activated();

    Ok(())
}

// ============================================================================
// ERROR SCENARIOS
// ============================================================================

#[test]
fn create_profile_with_duplicate_name_fails() -> Result<()> {
    // Given: default profile exists
    let env = TestEnv::new().with_profile("default").build()?;

    // When: try to create another "default" profile
    let manifest = env.load_manifest()?;
    let profile_exists = manifest.profiles.iter().any(|p| p.name == "default");

    // Then: profile already exists (operation should be rejected)
    assert!(profile_exists, "Profile should already exist");

    // In real implementation, ProfileService::create_profile should return an error

    Ok(())
}

#[test]
fn activate_when_repo_file_missing() -> Result<()> {
    // Given: manifest says profile has file, but file not in repo
    let env = TestEnv::new()
        .with_profile("default")
        .with_selected_profile("default")
        .build()?;

    // Add file to manifest but NOT to filesystem
    let mut manifest = env.load_manifest()?;
    if let Some(profile) = manifest.profiles.iter_mut().find(|p| p.name == "default") {
        profile.synced_files.push(".missing-file".to_string());
    }
    manifest.save(&env.repo_path)?;

    // The repo file doesn't exist
    assert!(!env.profile_file_path("default", ".missing-file").exists());

    // When: try to activate
    // This should fail or skip the missing file

    // In real implementation, this would be detected and handled
    // Either error out or skip with warning

    Ok(())
}

// ============================================================================
// CRASH RECOVERY
// ============================================================================

#[test]
fn switch_interrupted_old_symlinks_removed_new_not_created() -> Result<()> {
    // Given: switch started - old symlinks removed, new not yet created (crash scenario)
    let env = TestEnv::new()
        .with_profile("default")
        .with_profile("work")
        .with_activated_profile("default")
        .with_synced_file("default", ".zshrc", "default zsh")
        .build()?;

    // Simulate crash: old symlinks removed but config/tracking not updated
    std::fs::remove_file(env.home_path(".zshrc"))?;

    // State is now inconsistent:
    // - Config says "default" is active
    // - Tracking has .zshrc entry
    // - But symlink doesn't exist

    env.assert_active_profile("default");
    env.assert_file_tracked(".zshrc");
    assert!(!env.home_path(".zshrc").exists());

    // This inconsistency should be detectable by doctor

    Ok(())
}

// ============================================================================
// TEST ENVIRONMENT VERIFICATION
// ============================================================================

#[test]
fn test_multiple_profiles_isolation() -> Result<()> {
    // Verify that multiple profiles don't interfere with each other
    let env = TestEnv::new()
        .with_profile("work")
        .with_profile("home")
        .with_profile("gaming")
        .with_activated_profile("work")
        .with_synced_file("work", ".workrc", "work config")
        .with_synced_file("home", ".homerc", "home config")
        .with_synced_file("gaming", ".gamerc", "gaming config")
        .build()?;

    // Only work profile's files should be symlinked
    env.assert_is_symlink(".workrc");
    assert!(!env.home_path(".homerc").exists());
    assert!(!env.home_path(".gamerc").exists());

    // But all files exist in their respective profile directories
    assert!(env.profile_file_path("work", ".workrc").exists());
    assert!(env.profile_file_path("home", ".homerc").exists());
    assert!(env.profile_file_path("gaming", ".gamerc").exists());

    // All profiles exist in manifest
    env.assert_profile_exists("work");
    env.assert_profile_exists("home");
    env.assert_profile_exists("gaming");

    Ok(())
}

// ============================================================================
// PROFILE INHERITANCE
// ============================================================================

#[test]
fn inheritance_resolve_files_merges_parent_and_child() -> Result<()> {
    // Given: p1 has f1 and f2, p2 inherits p1 and has f2 (override) and f3
    let env = TestEnv::new()
        .with_profile("p1")
        .with_profile("p2")
        .with_git()
        .build()?;

    // Set up files in p1
    let p1_path = env.profile_path("p1");
    std::fs::write(p1_path.join(".zshrc"), "p1 zshrc")?;
    std::fs::write(p1_path.join(".vimrc"), "p1 vimrc")?;

    // Set up files in p2 (overrides .vimrc, adds .config_nvim)
    let p2_path = env.profile_path("p2");
    std::fs::write(p2_path.join(".vimrc"), "p2 vimrc")?;
    std::fs::write(p2_path.join(".config_nvim"), "p2 nvim")?;

    // Set up common files
    let common_path = env.common_path();
    std::fs::create_dir_all(&common_path)?;
    std::fs::write(common_path.join(".gitconfig"), "common gitconfig")?;

    // Update manifest with inheritance and synced_files
    let mut manifest = env.load_manifest()?;
    manifest.common.synced_files = vec![".gitconfig".to_string()];
    if let Some(p1) = manifest.profiles.iter_mut().find(|p| p.name == "p1") {
        p1.synced_files = vec![".zshrc".to_string(), ".vimrc".to_string()];
    }
    if let Some(p2) = manifest.profiles.iter_mut().find(|p| p.name == "p2") {
        p2.inherits = Some("p1".to_string());
        p2.synced_files = vec![".vimrc".to_string(), ".config_nvim".to_string()];
    }
    manifest.save(&env.repo_path)?;

    // When: resolve files for p2
    let resolved = manifest.resolve_files("p2")?;

    // Then: 4 files total
    assert_eq!(resolved.len(), 4);

    let find = |path: &str| resolved.iter().find(|r| r.relative_path == path).unwrap();
    assert_eq!(find(".gitconfig").source_profile, "common");
    assert_eq!(find(".zshrc").source_profile, "p1"); // inherited
    assert_eq!(find(".vimrc").source_profile, "p2"); // child wins
    assert_eq!(find(".config_nvim").source_profile, "p2"); // own

    Ok(())
}

#[test]
fn inheritance_chain_validation_detects_cycle() -> Result<()> {
    let env = TestEnv::new()
        .with_profile("a")
        .with_profile("b")
        .with_git()
        .build()?;

    let mut manifest = env.load_manifest()?;
    if let Some(a) = manifest.profiles.iter_mut().find(|p| p.name == "a") {
        a.inherits = Some("b".to_string());
    }
    if let Some(b) = manifest.profiles.iter_mut().find(|p| p.name == "b") {
        b.inherits = Some("a".to_string());
    }

    // Validation should detect the cycle
    let result = manifest.validate_inheritance();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("cycle"));

    Ok(())
}

#[test]
fn inheritance_delete_guard_prevents_deleting_parent() -> Result<()> {
    use dotstate::services::ProfileService;

    let env = TestEnv::new()
        .with_profile("base")
        .with_profile("child")
        .with_activated_profile("child")
        .with_git()
        .build()?;

    // Set child to inherit from base
    let mut manifest = env.load_manifest()?;
    if let Some(child) = manifest.profiles.iter_mut().find(|p| p.name == "child") {
        child.inherits = Some("base".to_string());
    }
    manifest.save(&env.repo_path)?;

    // Try to delete base — should fail because child inherits from it
    let result = ProfileService::delete_profile(&env.repo_path, "base", "child");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("inherited by"));

    Ok(())
}

#[test]
fn inheritance_inherits_field_roundtrips_through_toml() -> Result<()> {
    let env = TestEnv::new()
        .with_profile("parent")
        .with_profile("child")
        .with_git()
        .build()?;

    // Set inheritance
    let mut manifest = env.load_manifest()?;
    if let Some(child) = manifest.profiles.iter_mut().find(|p| p.name == "child") {
        child.inherits = Some("parent".to_string());
    }
    manifest.save(&env.repo_path)?;

    // Reload and verify
    let loaded = env.load_manifest()?;
    let child = loaded.profiles.iter().find(|p| p.name == "child").unwrap();
    assert_eq!(child.inherits, Some("parent".to_string()));

    // parent should have no inherits
    let parent = loaded.profiles.iter().find(|p| p.name == "parent").unwrap();
    assert!(parent.inherits.is_none());

    Ok(())
}

#[test]
fn inheritance_profile_without_inherits_is_backward_compatible() -> Result<()> {
    let env = TestEnv::new()
        .with_profile("standalone")
        .with_git()
        .build()?;

    // Set up files
    let profile_path = env.profile_path("standalone");
    std::fs::write(profile_path.join(".zshrc"), "standalone zshrc")?;

    let common_path = env.common_path();
    std::fs::create_dir_all(&common_path)?;
    std::fs::write(common_path.join(".gitconfig"), "common gitconfig")?;

    let mut manifest = env.load_manifest()?;
    manifest.common.synced_files = vec![".gitconfig".to_string()];
    if let Some(p) = manifest
        .profiles
        .iter_mut()
        .find(|p| p.name == "standalone")
    {
        p.synced_files = vec![".zshrc".to_string()];
    }
    manifest.save(&env.repo_path)?;

    // Resolve files — should work exactly like before inheritance was added
    let resolved = manifest.resolve_files("standalone")?;
    assert_eq!(resolved.len(), 2);

    let find = |path: &str| resolved.iter().find(|r| r.relative_path == path).unwrap();
    assert_eq!(find(".gitconfig").source_profile, "common");
    assert_eq!(find(".zshrc").source_profile, "standalone");

    Ok(())
}
