//! Profile activation/deactivation commands.

use super::common::{print_error, print_info};
use super::ProfileCommand;
use crate::config::Config;
use crate::icons::Icons;
use crate::services::ProfileService;
use crate::utils::symlink_manager::OperationStatus;
use crate::utils::SymlinkManager;
use anyhow::{Context, Result};

/// Execute a profile subcommand.
pub fn execute(command: ProfileCommand) -> Result<()> {
    match command {
        ProfileCommand::Current => cmd_current(),
        ProfileCommand::List => cmd_list(),
        ProfileCommand::Switch { name } => cmd_switch(name),
    }
}

/// Print the current profile name.
pub fn cmd_current() -> Result<()> {
    println!("{}", current_profile_name()?);
    Ok(())
}

fn current_profile_name() -> Result<String> {
    let config_path = crate::utils::get_config_path();
    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        print_error("Repository not configured. Please run 'dotstate' to set up repository.");
        std::process::exit(1);
    }

    if config.active_profile.is_empty() {
        print_error("No active profile is set.");
        std::process::exit(1);
    }

    Ok(config.active_profile)
}

/// List all profiles, marking the active profile.
pub fn cmd_list() -> Result<()> {
    for line in profile_list_lines()? {
        println!("{line}");
    }
    Ok(())
}

fn profile_list_lines() -> Result<Vec<String>> {
    let config_path = crate::utils::get_config_path();
    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        print_error("Repository not configured. Please run 'dotstate' to set up repository.");
        std::process::exit(1);
    }

    let icons = Icons::from_config(&config);
    let profiles = ProfileService::get_profiles(&config.repo_path)?;

    if profiles.is_empty() {
        return Ok(vec![format!("{} No profiles found.", icons.info())]);
    }

    Ok(profiles
        .into_iter()
        .map(|profile| {
            let icon = if profile.name == config.active_profile {
                icons.active_profile()
            } else {
                icons.inactive_profile()
            };
            format!("{icon} {}", profile.name)
        })
        .collect())
}

/// Switch to a different profile and activate it.
pub fn cmd_switch(name: String) -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let mut config =
        Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        print_error("Repository not configured. Please run 'dotstate' to set up repository.");
        std::process::exit(1);
    }

    let icons = Icons::from_config(&config);

    let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
        .context("Failed to load profile manifest")?;

    if !manifest.profiles.iter().any(|p| p.name == name) {
        eprintln!("{} Profile '{name}' not found.", icons.error());
        std::process::exit(1);
    }

    if config.active_profile == name && config.profile_activated {
        print_info(&format!("Already on profile '{name}'"));
        return Ok(());
    }

    if config.profile_activated {
        let result = ProfileService::switch_profile(
            &config.repo_path,
            &config.active_profile,
            &name,
            config.backup_enabled,
        )?;

        config.active_profile = name.clone();
        config.profile_activated = true;
        config
            .save(&config_path)
            .context("Failed to save configuration")?;

        println!("{} Switched to profile '{name}'", icons.success());
        println!(
            "   Removed {} symlinks, created {} symlinks",
            result.removed_count, result.created_count
        );
        return Ok(());
    }

    let resolved_files = manifest
        .resolve_files(&name)
        .context("Failed to resolve files for target profile")?;

    if resolved_files.is_empty() {
        eprintln!(
            "{} Target profile '{name}' has no synced files (including inherited/common).",
            icons.error()
        );
        std::process::exit(1);
    }

    let mut symlink_mgr =
        SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;
    let operations = symlink_mgr.activate_resolved(&name, &resolved_files)?;

    let success_count = operations
        .iter()
        .filter(|op| {
            matches!(
                op.status,
                OperationStatus::Success | OperationStatus::Skipped(_)
            )
        })
        .count();
    let failed_count = operations.len() - success_count;

    if failed_count > 0 {
        eprintln!(
            "{} Activated {success_count} files, {failed_count} failed",
            icons.warning()
        );
        for op in &operations {
            if let OperationStatus::Failed(msg) = &op.status {
                eprintln!("   {} {}: {}", icons.error(), op.target.display(), msg);
            }
        }
        std::process::exit(1);
    }

    config.active_profile = name.clone();
    config.profile_activated = true;
    config
        .save(&config_path)
        .context("Failed to save configuration")?;

    println!("{} Switched to profile '{name}'", icons.success());
    println!("   Activated {success_count} symlinks");

    Ok(())
}

/// Execute the activate command.
pub fn cmd_activate() -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let mut config =
        Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        let icons = Icons::from_config(&config);
        eprintln!(
            "{} Repository not configured. Please run 'dotstate' to set up repository.",
            icons.error()
        );
        std::process::exit(1);
    }

    let icons = Icons::from_config(&config);

    // Check if already activated
    if config.profile_activated {
        println!(
            "{} Profile '{}' is already activated.",
            icons.info(),
            config.active_profile
        );
        println!("   No action needed. Use 'dotstate deactivate' to restore original files.");
        return Ok(());
    }

    // Get active profile info from manifest
    let active_profile_name = config.active_profile.clone();
    let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
        .context("Failed to load profile manifest")?;

    // Resolve the full file list (inheritance chain + common, with overrides)
    let resolved_files = manifest
        .resolve_files(&active_profile_name)
        .context("Failed to resolve files for active profile")?;

    if resolved_files.is_empty() {
        eprintln!(
            "{} Active profile '{active_profile_name}' has no synced files (including inherited/common).",
            icons.error()
        );
        eprintln!(
            "{} Run 'dotstate' to select and sync files.",
            icons.lightbulb()
        );
        std::process::exit(1);
    }

    // Show inheritance chain if applicable
    if let Ok(chain) = manifest.inheritance_chain(&active_profile_name) {
        if chain.len() > 1 {
            println!("   Inheritance chain: {}", chain.join(" -> "));
        }
    }

    println!(
        "{} Activating profile '{active_profile_name}'...",
        icons.sync()
    );
    println!(
        "   This will create symlinks for {} files",
        resolved_files.len()
    );

    // Create SymlinkManager and activate with resolved files
    let mut symlink_mgr =
        SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

    let operations = symlink_mgr.activate_resolved(&active_profile_name, &resolved_files)?;

    // Report results
    // Count Success and Skipped as successful (Skipped = symlink already correct)
    let success_count = operations
        .iter()
        .filter(|op| {
            matches!(
                op.status,
                OperationStatus::Success | OperationStatus::Skipped(_)
            )
        })
        .count();
    let failed_count = operations.len() - success_count;

    if failed_count > 0 {
        eprintln!(
            "{} Activated {success_count} files, {failed_count} failed",
            icons.warning()
        );
        for op in &operations {
            if let OperationStatus::Failed(msg) = &op.status {
                eprintln!("   {} {}: {}", icons.error(), op.target.display(), msg);
            }
        }
        std::process::exit(1);
    } else {
        // Mark as activated in config
        config.profile_activated = true;
        config
            .save(&config_path)
            .context("Failed to save configuration")?;

        println!(
            "{} Successfully activated profile '{active_profile_name}'",
            icons.success()
        );
        println!("   {success_count} symlinks created");
    }

    Ok(())
}

/// Execute the deactivate command.
pub fn cmd_deactivate() -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let mut config =
        Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        let icons = Icons::from_config(&config);
        eprintln!(
            "{} Repository not configured. Please run 'dotstate' to set up repository.",
            icons.error()
        );
        std::process::exit(1);
    }

    let icons = Icons::from_config(&config);

    println!("{} Deactivating dotstate...", icons.sync());
    println!("   This will restore all files from the repository");

    // Create SymlinkManager
    let mut symlink_mgr =
        SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

    // Deactivate all symlinks (profile + common), always restore files
    let operations = symlink_mgr.deactivate_profile_with_restore(&config.active_profile, true)?;

    // Report results
    // Count Success and Skipped as successful (Skipped = symlink already gone or not our symlink)
    let success_count = operations
        .iter()
        .filter(|op| {
            matches!(
                op.status,
                OperationStatus::Success | OperationStatus::Skipped(_)
            )
        })
        .count();
    let failed_count = operations.len() - success_count;

    if operations.is_empty() {
        println!(
            "{} No symlinks were tracked. Nothing to deactivate.",
            icons.info()
        );
    } else if failed_count > 0 {
        eprintln!(
            "{} Deactivated {success_count} files, {failed_count} failed",
            icons.warning()
        );
        for op in &operations {
            if let OperationStatus::Failed(msg) = &op.status {
                eprintln!("   {} {}: {}", icons.error(), op.target.display(), msg);
            }
        }
        std::process::exit(1);
    } else {
        // Mark as deactivated in config
        config.profile_activated = false;
        config
            .save(&config_path)
            .context("Failed to save configuration")?;

        println!("{} Successfully deactivated dotstate", icons.success());
        println!("   {success_count} files restored");
        println!(
            "{} Dotstate is now deactivated. Use 'dotstate activate' to reactivate.",
            icons.lightbulb()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{cmd_switch, current_profile_name, profile_list_lines};
    use crate::config::{Config, RepoMode};
    use crate::utils::profile_manifest::{ProfileInfo, ProfileManifest};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use tempfile::TempDir;

    struct TestPaths {
        _root: TempDir,
        repo: PathBuf,
        home: PathBuf,
        config: PathBuf,
        backup: PathBuf,
    }

    struct EnvGuard {
        old_home: Option<String>,
        old_config: Option<String>,
        old_backup: Option<String>,
    }

    impl EnvGuard {
        fn set(paths: &TestPaths) -> Self {
            let old_home = std::env::var("DOTSTATE_TEST_HOME").ok();
            let old_config = std::env::var("DOTSTATE_TEST_CONFIG_DIR").ok();
            let old_backup = std::env::var("DOTSTATE_TEST_BACKUP_DIR").ok();

            std::env::set_var("DOTSTATE_TEST_HOME", &paths.home);
            std::env::set_var("DOTSTATE_TEST_CONFIG_DIR", &paths.config);
            std::env::set_var("DOTSTATE_TEST_BACKUP_DIR", &paths.backup);

            Self {
                old_home,
                old_config,
                old_backup,
            }
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
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

    fn setup_paths() -> anyhow::Result<TestPaths> {
        let root = TempDir::new()?;
        let repo = root.path().join("repo");
        let home = root.path().join("home");
        let config = root.path().join("config");
        let backup = root.path().join("backup");

        fs::create_dir_all(repo.join(".git"))?;
        fs::create_dir_all(&home)?;
        fs::create_dir_all(&config)?;
        fs::create_dir_all(&backup)?;

        Ok(TestPaths {
            _root: root,
            repo,
            home,
            config,
            backup,
        })
    }

    fn write_config(
        repo: &Path,
        active_profile: &str,
        profile_activated: bool,
    ) -> anyhow::Result<()> {
        let config_path = crate::utils::get_config_path();
        let config = Config {
            repo_mode: RepoMode::Local,
            repo_path: repo.to_path_buf(),
            active_profile: active_profile.to_string(),
            profile_activated,
            backup_enabled: false,
            icon_set: "unicode".to_string(),
            ..Config::default()
        };
        config.save(&config_path)
    }

    fn write_manifest(repo: &Path) -> anyhow::Result<()> {
        let manifest = ProfileManifest {
            profiles: vec![
                ProfileInfo {
                    name: "default".to_string(),
                    description: None,
                    inherits: None,
                    synced_files: vec![".default-file".to_string()],
                    packages: Vec::new(),
                },
                ProfileInfo {
                    name: "work".to_string(),
                    description: None,
                    inherits: None,
                    synced_files: vec![".work-file".to_string()],
                    packages: Vec::new(),
                },
            ],
            ..ProfileManifest::default()
        };
        fs::create_dir_all(repo.join("default"))?;
        fs::create_dir_all(repo.join("work"))?;
        fs::write(
            repo.join("default").join(".default-file"),
            "default content",
        )?;
        fs::write(repo.join("work").join(".work-file"), "work content")?;
        manifest.save(repo)
    }

    #[test]
    fn current_profile_reads_configured_profile() -> anyhow::Result<()> {
        let _lock = env_lock().lock().unwrap();
        let paths = setup_paths()?;
        let _guard = EnvGuard::set(&paths);
        write_manifest(&paths.repo)?;
        write_config(&paths.repo, "work", false)?;

        assert_eq!(current_profile_name()?, "work");
        Ok(())
    }

    #[test]
    fn profile_list_marks_active_and_inactive_profiles() -> anyhow::Result<()> {
        let _lock = env_lock().lock().unwrap();
        let paths = setup_paths()?;
        let _guard = EnvGuard::set(&paths);
        write_manifest(&paths.repo)?;
        write_config(&paths.repo, "work", false)?;

        let lines = profile_list_lines()?;

        assert_eq!(lines.len(), 2);
        assert!(lines.iter().any(|line| line.starts_with("○ default")));
        assert!(lines.iter().any(|line| line.starts_with("★ work")));
        Ok(())
    }

    #[test]
    fn profile_list_handles_empty_manifest() -> anyhow::Result<()> {
        let _lock = env_lock().lock().unwrap();
        let paths = setup_paths()?;
        let _guard = EnvGuard::set(&paths);
        let manifest = ProfileManifest::default();
        manifest.save(&paths.repo)?;
        write_config(&paths.repo, "default", false)?;

        let lines = profile_list_lines()?;

        assert_eq!(lines, vec!["ℹ No profiles found.".to_string()]);
        Ok(())
    }

    #[test]
    fn switch_activates_target_profile_when_deactivated() -> anyhow::Result<()> {
        let _lock = env_lock().lock().unwrap();
        let paths = setup_paths()?;
        let _guard = EnvGuard::set(&paths);
        write_manifest(&paths.repo)?;
        write_config(&paths.repo, "default", false)?;

        cmd_switch("work".to_string())?;

        let config = Config::load_or_create(&crate::utils::get_config_path())?;
        assert_eq!(config.active_profile, "work");
        assert!(config.profile_activated);

        let target = paths.home.join(".work-file");
        assert!(target.is_symlink());
        assert_eq!(
            fs::read_link(target)?,
            paths.repo.join("work").join(".work-file")
        );
        Ok(())
    }

    #[test]
    fn switch_replaces_symlinks_when_current_profile_is_active() -> anyhow::Result<()> {
        let _lock = env_lock().lock().unwrap();
        let paths = setup_paths()?;
        let _guard = EnvGuard::set(&paths);
        write_manifest(&paths.repo)?;
        write_config(&paths.repo, "default", true)?;

        let manifest = ProfileManifest::load_or_backfill(&paths.repo)?;
        let default_files = manifest.resolve_files("default")?;
        let mut symlink_mgr =
            crate::utils::SymlinkManager::new_with_backup(paths.repo.clone(), false)?;
        symlink_mgr.activate_resolved("default", &default_files)?;

        cmd_switch("work".to_string())?;

        let config = Config::load_or_create(&crate::utils::get_config_path())?;
        assert_eq!(config.active_profile, "work");
        assert!(config.profile_activated);

        assert!(!paths.home.join(".default-file").exists());
        let target = paths.home.join(".work-file");
        assert!(target.is_symlink());
        assert_eq!(
            fs::read_link(target)?,
            paths.repo.join("work").join(".work-file")
        );
        Ok(())
    }
}
