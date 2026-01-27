use crate::config::Config;
use crate::git::GitManager;
use crate::utils::SymlinkManager;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::{info, warn};

/// A friendly TUI tool for managing dotfiles with GitHub sync
#[derive(Parser, Debug)]
#[command(name = "dotstate", version, about = "A friendly TUI tool for managing dotfiles with GitHub sync", long_about = None, disable_help_subcommand = true, arg_required_else_help = false)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Disable colors in the TUI (also respects NO_COLOR env var)
    #[arg(long, global = true)]
    pub no_colors: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Sync with remote: commit, pull (with rebase), and push
    Sync {
        /// Custom commit message
        #[arg(short, long)]
        message: Option<String>,
    },
    /// List all synced files
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
    /// Add a file to sync
    Add {
        /// Path to the file to add
        path: PathBuf,
        /// Add as a common file (shared across all profiles)
        #[arg(long)]
        common: bool,
    },
    /// Remove a file from sync
    Remove {
        /// Path to the file to remove (relative to home directory, e.g., ".zshrc")
        path: String,
        /// Remove from common files (shared across all profiles)
        #[arg(long)]
        common: bool,
    },
    /// Activate the symlinks, restores app state after deactivation.
    Activate,
    /// Deactivate symlinks. this might be useful if you are going to uninstall dotstate or you need the original files.
    Deactivate,
    /// Run diagnostics and optionally fix issues
    Doctor {
        /// Attempt to auto-fix detected issues
        #[arg(long)]
        fix: bool,
        /// Show detailed diagnostic information
        #[arg(short, long)]
        verbose: bool,
        /// Output results as JSON for scripting
        #[arg(long)]
        json: bool,
    },
    /// Shows logs location and how to view them
    Logs,
    /// Configuration file location
    Config,
    /// Repository location
    Repository,
    /// Show help for a specific command
    Help {
        /// Command to show help for
        command: Option<String>,
    },
    /// Check for updates and optionally upgrade DotState
    Upgrade {
        /// Check for updates without prompting to install
        #[arg(long)]
        check: bool,
    },
    /// Manage packages for profiles
    Packages {
        #[command(subcommand)]
        command: PackagesCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum PackagesCommand {
    /// List packages for a profile
    List {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Show detailed package information
        #[arg(short, long)]
        verbose: bool,
    },
    /// Add a package to a profile
    Add {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Package display name
        #[arg(short, long)]
        name: Option<String>,
        /// Package manager (brew, cargo, apt, npm, pip, custom, etc.)
        #[arg(short, long)]
        manager: Option<String>,
        /// Binary name to check for existence
        #[arg(short, long)]
        binary: Option<String>,
        /// Optional description
        #[arg(long)]
        description: Option<String>,
        /// Package name in the manager (defaults to binary name)
        #[arg(long)]
        package_name: Option<String>,
        /// Install command (required for custom manager)
        #[arg(long)]
        install_command: Option<String>,
        /// Command to check if package exists (optional, for custom)
        #[arg(long)]
        existence_check: Option<String>,
    },
    /// Remove a package from a profile
    Remove {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
        /// Package name to remove
        name: Option<String>,
    },
    /// Check installation status of packages
    Check {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Install all missing packages
    Install {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Show package manager output
        #[arg(short, long)]
        verbose: bool,
    },
}

impl Cli {
    /// Execute the CLI command
    pub fn execute(self) -> Result<()> {
        match self.command {
            Some(Commands::Sync { message }) => Self::cmd_sync(message),
            Some(Commands::List { verbose }) => Self::cmd_list(verbose),
            Some(Commands::Add { path, common }) => Self::cmd_add(path, common),
            Some(Commands::Remove { path, common }) => Self::cmd_remove(path, common),
            Some(Commands::Activate) => Self::cmd_activate(),
            Some(Commands::Deactivate) => Self::cmd_deactivate(),
            Some(Commands::Doctor { fix, verbose, json }) => Self::cmd_doctor(fix, verbose, json),
            Some(Commands::Help { command }) => Self::cmd_help(command),
            Some(Commands::Logs) => Self::cmd_logs(),
            Some(Commands::Config) => Self::cmd_config(),
            Some(Commands::Repository) => Self::cmd_repository(),
            Some(Commands::Upgrade { check }) => Self::cmd_upgrade(check),
            Some(Commands::Packages { command }) => Self::cmd_packages(command),
            None => {
                // No command provided, launch TUI
                Ok(())
            }
        }
    }

    fn cmd_sync(message: Option<String>) -> Result<()> {
        use crate::config::RepoMode;

        info!("CLI: sync command executed");
        let config_path = crate::utils::get_config_path();

        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        // Check if repository is configured (either GitHub or Local mode)
        if !config.is_repo_configured() {
            warn!("CLI sync: Repository not configured");
            eprintln!(
                "❌ Repository not configured. Please run 'dotstate' to set up repository sync."
            );
            std::process::exit(1);
        }

        let repo_path = &config.repo_path;
        let git_mgr = GitManager::open_or_init(repo_path).context("Failed to open repository")?;

        let branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| config.default_branch.clone());

        // Get token based on repo mode (None for Local mode)
        let token_string = match config.repo_mode {
            RepoMode::Local => None,
            RepoMode::GitHub => config.get_github_token(),
        };
        let token = token_string.as_deref();

        // Only require token for GitHub mode
        if matches!(config.repo_mode, RepoMode::GitHub) && token.is_none() {
            eprintln!("❌ GitHub token not found.");
            eprintln!();
            eprintln!("Please provide a GitHub token using one of these methods:");
            eprintln!("  1. Set the DOTSTATE_GITHUB_TOKEN environment variable:");
            eprintln!("     export DOTSTATE_GITHUB_TOKEN=ghp_your_token_here");
            eprintln!("  2. Configure it in the TUI by running 'dotstate'");
            eprintln!();
            eprintln!("Create a token at: https://github.com/settings/tokens");
            eprintln!("Required scope: repo (full control of private repositories)");
            std::process::exit(1);
        }

        println!("📝 Committing changes...");
        let commit_msg = message.unwrap_or_else(|| {
            git_mgr
                .generate_commit_message()
                .unwrap_or_else(|_| "Update dotfiles".to_string())
        });
        git_mgr
            .commit_all(&commit_msg)
            .context("Failed to commit changes")?;

        println!("📥 Pulling changes from remote...");
        let pulled_count = git_mgr
            .pull_with_rebase("origin", &branch, token)
            .context("Failed to pull from remote")?;

        let push_dest = match config.repo_mode {
            RepoMode::GitHub => "GitHub",
            RepoMode::Local => "remote",
        };
        println!("📤 Pushing to {}...", push_dest);
        git_mgr
            .push("origin", &branch, token)
            .context("Failed to push to remote")?;

        if pulled_count > 0 {
            info!("CLI sync completed: pulled {} commit(s)", pulled_count);
            println!(
                "✅ Successfully synced with remote! Pulled {} change(s) from remote.",
                pulled_count
            );

            // Ensure symlinks for any new files pulled from remote
            // This is efficient - only creates symlinks for missing files
            use crate::services::ProfileService;
            println!("🔗 Checking for new files to symlink...");
            match ProfileService::ensure_profile_symlinks(
                repo_path,
                &config.active_profile,
                config.backup_enabled,
            ) {
                Ok((created, _skipped, errors)) => {
                    if created > 0 {
                        println!("   Created {} symlink(s) for new files.", created);
                    } else {
                        println!("   All files already have symlinks.");
                    }
                    if !errors.is_empty() {
                        eprintln!("⚠️  Warning: {} error(s) creating symlinks:", errors.len());
                        for error in errors {
                            eprintln!("   {}", error);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to ensure symlinks after pull: {}", e);
                    eprintln!(
                        "⚠️  Warning: Failed to create symlinks for new files: {}",
                        e
                    );
                }
            }

            // Also ensure common symlinks
            match ProfileService::ensure_common_symlinks(repo_path, config.backup_enabled) {
                Ok((created, _skipped, errors)) => {
                    if created > 0 {
                        println!("   Created {} common symlink(s).", created);
                    }
                    if !errors.is_empty() {
                        eprintln!(
                            "⚠️  Warning: {} error(s) creating common symlinks:",
                            errors.len()
                        );
                        for error in errors {
                            eprintln!("   {}", error);
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to ensure common symlinks after pull: {}", e);
                    eprintln!("⚠️  Warning: Failed to create common symlinks: {}", e);
                }
            }
        } else {
            info!("CLI sync completed: no changes pulled");
            println!("✅ Successfully synced with remote! No changes pulled from remote.");
        }
        Ok(())
    }

    fn cmd_logs() -> Result<()> {
        let log_dir = dirs::cache_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
            .join("dotstate");
        let log_file = log_dir.join("dotstate.log");
        println!("{}", log_file.display());
        Ok(())
    }

    fn cmd_config() -> Result<()> {
        let config_path = crate::utils::get_config_path();
        println!("{}", config_path.display());
        Ok(())
    }

    fn cmd_repository() -> Result<()> {
        let repo_path =
            crate::utils::get_repository_path().context("Failed to get repository path")?;
        println!("{}", repo_path.display());
        Ok(())
    }

    fn cmd_list(verbose: bool) -> Result<()> {
        let config_path = crate::utils::get_config_path();

        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if !config.profile_activated {
            eprintln!("⚠️  Profile is not activated. Please activate your profile first:");
            eprintln!("   dotstate activate");
            eprintln!("\n   This ensures your symlinks are active before listing files.");
            std::process::exit(1);
        }

        // Get manifest
        let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
            .context("Failed to load profile manifest")?;

        // Get common files
        let common_files = manifest.get_common_files();

        // Get active profile's synced files
        let empty_vec = Vec::new();
        let synced_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == config.active_profile)
            .map(|p| &p.synced_files)
            .unwrap_or(&empty_vec);

        if common_files.is_empty() && synced_files.is_empty() {
            println!("No files are currently synced.");
            return Ok(());
        }

        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let repo_path = &config.repo_path;
        let profile_name = &config.active_profile;

        // Print common files first
        if !common_files.is_empty() {
            println!(
                "Common files ({}) - shared across all profiles:",
                common_files.len()
            );
            for file in common_files {
                let symlink_path = home_dir.join(file);
                let repo_file_path = repo_path.join("common").join(file);

                if verbose {
                    let symlink_exists = symlink_path.exists();
                    let repo_file_exists = repo_file_path.exists();

                    println!("  {}", file);
                    println!("    Symlink:   {}", symlink_path.display());
                    if symlink_exists {
                        if let Ok(metadata) = symlink_path.symlink_metadata() {
                            if metadata.is_symlink() {
                                println!("      Status:  ✓ Active symlink");
                            } else {
                                println!("      Status:  ⚠ File exists but is not a symlink");
                            }
                        } else {
                            println!("      Status:  ✓ Exists");
                        }
                    } else {
                        println!("      Status:  ✗ Not found");
                    }
                    println!("    Storage:   {}", repo_file_path.display());
                    if repo_file_exists {
                        println!("      Status:  ✓ Exists");
                    } else {
                        println!("      Status:  ✗ Not found");
                    }
                } else {
                    println!("  {}", file);
                    println!("    Symlink:   {}", symlink_path.display());
                    println!("    Storage:   {}", repo_file_path.display());
                }
            }
            println!();
        }

        // Print profile files
        if !synced_files.is_empty() {
            println!("Profile files ({}) - {}:", synced_files.len(), profile_name);
            for file in synced_files {
                let symlink_path = home_dir.join(file);
                let repo_file_path = repo_path.join(profile_name).join(file);

                if verbose {
                    let symlink_exists = symlink_path.exists();
                    let repo_file_exists = repo_file_path.exists();

                    println!("  {}", file);
                    println!("    Symlink:   {}", symlink_path.display());
                    if symlink_exists {
                        if let Ok(metadata) = symlink_path.symlink_metadata() {
                            if metadata.is_symlink() {
                                println!("      Status:  ✓ Active symlink");
                            } else {
                                println!("      Status:  ⚠ File exists but is not a symlink");
                            }
                        } else {
                            println!("      Status:  ✓ Exists");
                        }
                    } else {
                        println!("      Status:  ✗ Not found");
                    }
                    println!("    Storage:   {}", repo_file_path.display());
                    if repo_file_exists {
                        println!("      Status:  ✓ Exists");
                    } else {
                        println!("      Status:  ✗ Not found");
                    }
                } else {
                    println!("  {}", file);
                    println!("    Symlink:   {}", symlink_path.display());
                    println!("    Storage:   {}", repo_file_path.display());
                }
            }
        }

        Ok(())
    }

    fn cmd_add(path: PathBuf, common: bool) -> Result<()> {
        use crate::services::{AddFileResult, SyncService};

        let config_path = crate::utils::get_config_path();
        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        // Resolve path relative to home directory
        let home = dirs::home_dir().context("Failed to get home directory")?;

        let resolved_path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()?.join(path)
        };

        if !resolved_path.exists() {
            eprintln!("❌ File not found: {:?}", resolved_path);
            std::process::exit(1);
        }

        // Get relative path from home
        let relative_path = resolved_path
            .strip_prefix(&home)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| resolved_path.clone());
        let relative_str = relative_path.to_string_lossy().to_string();

        // Show confirmation prompt
        let destination = if common { "common files" } else { "profile" };
        println!(
            "⚠️  Warning: This will move the following path to {} and replace it with a symlink:",
            destination
        );
        println!("   {}", resolved_path.display());
        if common {
            println!("\n   This file will be shared across ALL profiles.");
        }
        println!("\n   Make sure you know what you are doing.");
        print!("   Continue? [y/N]: ");
        use std::io::{self, Write};
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;

        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("Cancelled.");
            return Ok(());
        }

        info!(
            "CLI: Adding file to sync: {} (common: {})",
            relative_str, common
        );

        // Use appropriate SyncService method
        let result = if common {
            SyncService::add_common_file_to_sync(
                &config,
                &resolved_path,
                &relative_str,
                config.backup_enabled,
            )?
        } else {
            SyncService::add_file_to_sync(
                &config,
                &resolved_path,
                &relative_str,
                config.backup_enabled,
            )?
        };

        match result {
            AddFileResult::Success => {
                // Check if this is a custom file (not in default dotfile candidates)
                if !common && SyncService::is_custom_file(&relative_str) {
                    // Add to config.custom_files if not already there
                    let mut config = Config::load_or_create(&config_path)
                        .context("Failed to load configuration")?;
                    if !config.custom_files.contains(&relative_str) {
                        config.custom_files.push(relative_str.clone());
                        config.save(&config_path)?;
                    }
                }
                let dest_type = if common { "common files" } else { "repository" };
                println!(
                    "✅ Added {} to {} and created symlink",
                    relative_str, dest_type
                );
            }
            AddFileResult::AlreadySynced => {
                let dest_type = if common { "common" } else { "synced" };
                println!("ℹ️  File is already {}: {}", dest_type, relative_str);
            }
            AddFileResult::ValidationFailed(msg) => {
                eprintln!("❌ {}", msg);
                std::process::exit(1);
            }
        }

        Ok(())
    }

    fn cmd_remove(path: String, common: bool) -> Result<()> {
        use crate::services::{RemoveFileResult, SyncService};

        let config_path = crate::utils::get_config_path();
        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        // Show confirmation prompt
        let source = if common { "common files" } else { "profile" };
        println!(
            "⚠️  Warning: This will remove {} from {} and restore the original file.",
            path, source
        );
        print!("   Continue? [y/N]: ");
        use std::io::{self, Write};
        io::stdout().flush().context("Failed to flush stdout")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read input")?;

        let trimmed = input.trim().to_lowercase();
        if trimmed != "y" && trimmed != "yes" {
            println!("Cancelled.");
            return Ok(());
        }

        info!(
            "CLI: Removing file from sync: {} (common: {})",
            path, common
        );

        // Use appropriate SyncService method
        let result = if common {
            SyncService::remove_common_file_from_sync(&config, &path)?
        } else {
            SyncService::remove_file_from_sync(&config, &path)?
        };

        match result {
            RemoveFileResult::Success => {
                // Remove from config.custom_files if present
                if !common {
                    let mut config = Config::load_or_create(&config_path)
                        .context("Failed to load configuration")?;
                    config.custom_files.retain(|f| f != &path);
                    config.save(&config_path)?;
                }
                let source_type = if common { "common files" } else { "sync" };
                println!(
                    "✅ Removed {} from {} and restored original file",
                    path, source_type
                );
            }
            RemoveFileResult::NotSynced => {
                let source_type = if common { "common" } else { "synced" };
                println!("ℹ️  File is not {}: {}", source_type, path);
            }
        }

        Ok(())
    }

    fn cmd_activate() -> Result<()> {
        let config_path = crate::utils::get_config_path();
        let mut config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if !config.is_repo_configured() {
            eprintln!("❌ Repository not configured. Please run 'dotstate' to set up repository.");
            std::process::exit(1);
        }

        // Check if already activated
        if config.profile_activated {
            println!(
                "ℹ️  Profile '{}' is already activated.",
                config.active_profile
            );
            println!("   No action needed. Use 'dotstate deactivate' to restore original files.");
            return Ok(());
        }

        // Get active profile info from manifest
        let active_profile_name = config.active_profile.clone();
        let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
            .context("Failed to load profile manifest")?;
        let active_profile_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == active_profile_name)
            .ok_or_else(|| anyhow::anyhow!("No active profile found"))?
            .synced_files
            .clone();

        if active_profile_files.is_empty() {
            eprintln!(
                "❌ Active profile '{}' has no synced files.",
                active_profile_name
            );
            eprintln!("💡 Run 'dotstate' to select and sync files.");
            std::process::exit(1);
        }

        println!("🔗 Activating profile '{}'...", active_profile_name);
        println!(
            "   This will create symlinks for {} files",
            active_profile_files.len()
        );

        // Create SymlinkManager
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

        // Activate profile files
        let mut operations =
            symlink_mgr.activate_profile(&active_profile_name, &active_profile_files)?;

        // Also activate common files if any exist
        let common_files: Vec<String> = manifest.get_common_files().to_vec();
        if !common_files.is_empty() {
            let common_operations = symlink_mgr.activate_common_files(&common_files)?;
            operations.extend(common_operations);
        }

        // Report results
        // Count Success and Skipped as successful (Skipped = symlink already correct)
        let success_count = operations
            .iter()
            .filter(|op| {
                matches!(
                    op.status,
                    crate::utils::symlink_manager::OperationStatus::Success
                        | crate::utils::symlink_manager::OperationStatus::Skipped(_)
                )
            })
            .count();
        let failed_count = operations.len() - success_count;

        if failed_count > 0 {
            eprintln!(
                "⚠️  Activated {} files, {} failed",
                success_count, failed_count
            );
            for op in &operations {
                if let crate::utils::symlink_manager::OperationStatus::Failed(msg) = &op.status {
                    eprintln!("   ❌ {}: {}", op.target.display(), msg);
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
                "✅ Successfully activated profile '{}'",
                active_profile_name
            );
            println!("   {} symlinks created", success_count);
        }

        Ok(())
    }

    fn cmd_deactivate() -> Result<()> {
        let config_path = crate::utils::get_config_path();
        let mut config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if !config.is_repo_configured() {
            eprintln!("❌ Repository not configured. Please run 'dotstate' to set up repository.");
            std::process::exit(1);
        }

        println!("🔓 Deactivating dotstate...");
        println!("   This will restore all files from the repository");

        // Create SymlinkManager
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

        // Deactivate all symlinks (profile + common), always restore files
        let operations =
            symlink_mgr.deactivate_profile_with_restore(&config.active_profile, true)?;

        // Report results
        // Count Success and Skipped as successful (Skipped = symlink already gone or not our symlink)
        let success_count = operations
            .iter()
            .filter(|op| {
                matches!(
                    op.status,
                    crate::utils::symlink_manager::OperationStatus::Success
                        | crate::utils::symlink_manager::OperationStatus::Skipped(_)
                )
            })
            .count();
        let failed_count = operations.len() - success_count;

        if operations.is_empty() {
            println!("ℹ️  No symlinks were tracked. Nothing to deactivate.");
        } else if failed_count > 0 {
            eprintln!(
                "⚠️  Deactivated {} files, {} failed",
                success_count, failed_count
            );
            for op in &operations {
                if let crate::utils::symlink_manager::OperationStatus::Failed(msg) = &op.status {
                    eprintln!("   ❌ {}: {}", op.target.display(), msg);
                }
            }
            std::process::exit(1);
        } else {
            // Mark as deactivated in config
            config.profile_activated = false;
            config
                .save(&config_path)
                .context("Failed to save configuration")?;

            println!("✅ Successfully deactivated dotstate");
            println!("   {} files restored", success_count);
            println!("💡 Dotstate is now deactivated. Use 'dotstate activate' to reactivate.");
        }

        Ok(())
    }

    fn cmd_doctor(fix: bool, verbose: bool, json: bool) -> Result<()> {
        use crate::utils::doctor::{Doctor, DoctorOptions};

        let config_path = crate::utils::get_config_path();
        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if !config.is_repo_configured() {
            if json {
                println!(
                    r#"{{"error": "Repository not configured", "suggestion": "Run 'dotstate' to set up repository"}}"#
                );
            } else {
                eprintln!(
                    "❌ Repository not configured. Please run 'dotstate' to set up repository."
                );
            }
            std::process::exit(1);
        }

        let options = DoctorOptions {
            fix_mode: fix,
            verbose,
            json_output: json,
        };

        let mut doctor = Doctor::new(config, options);
        let report = doctor.run_diagnostics()?;

        if json {
            println!("{}", serde_json::to_string_pretty(&report)?);
        } else {
            // Summary is printed by doctor itself
        }

        if report.summary.errors > 0 {
            std::process::exit(1);
        }

        Ok(())
    }

    fn cmd_help(command: Option<String>) -> Result<()> {
        use clap::CommandFactory;

        if let Some(cmd_name) = command {
            // Show help for a specific command
            let mut cli = Cli::command();
            if let Some(subcommand) = cli.find_subcommand_mut(&cmd_name) {
                let help = subcommand.render_help();
                println!("{}", help);
            } else {
                eprintln!("❌ Unknown command: {}", cmd_name);
                eprintln!("\nAvailable commands:");
                Self::print_all_commands();
                std::process::exit(1);
            }
        } else {
            // Show list of all available commands
            println!("Available commands:\n");
            Self::print_all_commands();
            println!(
                "\nUse 'dotstate help <command>' to see detailed help for a specific command."
            );
        }
        Ok(())
    }

    fn cmd_upgrade(check_only: bool) -> Result<()> {
        use crate::version_check::{check_for_updates_now, current_version, UpdateInfo};

        println!("🔍 Checking for updates...");
        println!("   Current version: {}", current_version());
        println!();

        match check_for_updates_now() {
            Some(update_info) => {
                println!(
                    "🎉 New version available: {} (current: {})",
                    update_info.latest_version, update_info.current_version
                );
                println!();
                println!("📝 Release notes: {}", update_info.release_url);
                println!();

                if check_only {
                    // Just show update options without prompting
                    println!("Update options:");
                    println!();
                    println!("  1. Using install script:");
                    println!(
                        "     curl -fsSL {} | bash",
                        UpdateInfo::install_script_url()
                    );
                    println!();
                    println!("  2. Using Cargo:");
                    println!("     cargo install dotstate --force");
                    println!();
                    println!("  3. Using Homebrew:");
                    println!("     brew upgrade dotstate");
                    println!();
                    println!("Run 'dotstate upgrade' (without --check) for interactive upgrade.");
                    return Ok(());
                }

                // Interactive mode
                println!("How would you like to update?");
                println!();
                println!("  1. Run install script (recommended)");
                println!("     Downloads and installs the latest binary.");
                println!("     ⚠️  Warning: May conflict with cargo/brew installations.");
                println!();
                println!("  2. Show manual update commands");
                println!("     Display commands for cargo, brew, or manual download.");
                println!();
                println!("  3. Cancel");
                println!();
                print!("Enter choice [1-3]: ");

                use std::io::{self, Write};
                io::stdout().flush().context("Failed to flush stdout")?;

                let mut input = String::new();
                io::stdin()
                    .read_line(&mut input)
                    .context("Failed to read input")?;

                let choice = input.trim();

                match choice {
                    "1" => {
                        println!();
                        println!("⚠️  This will download and run the install script from:");
                        println!("   {}", UpdateInfo::install_script_url());
                        println!();
                        println!("   This may conflict with cargo or homebrew installations.");
                        println!("   If you installed via cargo or brew, consider using those to update.");
                        println!();
                        print!("Continue? [y/N]: ");
                        io::stdout().flush().context("Failed to flush stdout")?;

                        let mut confirm = String::new();
                        io::stdin()
                            .read_line(&mut confirm)
                            .context("Failed to read input")?;

                        let confirmed = confirm.trim().to_lowercase();
                        if confirmed != "y" && confirmed != "yes" {
                            println!("Cancelled.");
                            return Ok(());
                        }

                        println!();
                        println!("📥 Running install script...");
                        println!();

                        // Run the install script
                        let status = std::process::Command::new("bash")
                            .arg("-c")
                            .arg(format!(
                                "curl -fsSL {} | bash",
                                UpdateInfo::install_script_url()
                            ))
                            .status()
                            .context("Failed to run install script")?;

                        if status.success() {
                            println!();
                            println!(
                                "✅ Update complete! Please restart dotstate to use the new version."
                            );
                        } else {
                            eprintln!();
                            eprintln!(
                                "❌ Install script failed with exit code: {}",
                                status.code().unwrap_or(-1)
                            );
                            eprintln!("   Try updating manually using one of the other methods.");
                            std::process::exit(1);
                        }
                    }
                    "2" => {
                        println!();
                        println!("Manual update options:");
                        println!();
                        println!("  Using install script:");
                        println!("    curl -fsSL {} | bash", UpdateInfo::install_script_url());
                        println!();
                        println!("  Using Cargo:");
                        println!("    cargo install dotstate --force");
                        println!();
                        println!("  Using Homebrew:");
                        println!("    brew upgrade dotstate");
                        println!();
                        println!("  Direct download:");
                        println!("    {}", UpdateInfo::releases_url());
                    }
                    "3" | "" => {
                        println!("Cancelled.");
                    }
                    _ => {
                        println!("Invalid choice. Cancelled.");
                    }
                }
            }
            None => {
                println!(
                    "✅ You're running the latest version ({})!",
                    current_version()
                );
            }
        }

        Ok(())
    }

    fn cmd_packages(command: PackagesCommand) -> Result<()> {
        match command {
            PackagesCommand::List { profile, verbose } => Self::cmd_packages_list(profile, verbose),
            PackagesCommand::Add {
                profile,
                name,
                manager,
                binary,
                description,
                package_name,
                install_command,
                existence_check,
            } => Self::cmd_packages_add(
                profile,
                name,
                manager,
                binary,
                description,
                package_name,
                install_command,
                existence_check,
            ),
            PackagesCommand::Remove { profile, yes, name } => {
                Self::cmd_packages_remove(profile, yes, name)
            }
            PackagesCommand::Check { profile } => Self::cmd_packages_check(profile),
            PackagesCommand::Install { profile, verbose } => {
                println!(
                    "packages install (profile: {:?}, verbose: {})",
                    profile, verbose
                );
                Ok(())
            }
        }
    }

    fn cmd_packages_list(profile: Option<String>, verbose: bool) -> Result<()> {
        use crate::cli::common::{print_error, CliContext};
        use crate::services::package_service::{PackageCheckStatus, PackageService};

        let ctx = CliContext::load()?;
        let profile_name = ctx.resolve_profile(profile.as_deref());

        // Validate profile exists
        if !ctx.profile_exists(&profile_name) {
            print_error(&format!("Profile '{}' not found", profile_name));
            std::process::exit(1);
        }

        let is_active = ctx.is_active_profile(&profile_name);
        let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

        if packages.is_empty() {
            println!("No packages configured for profile '{}'", profile_name);
            println!("Use 'dotstate packages add' to add packages.");
            return Ok(());
        }

        println!("Packages for profile '{}':\n", profile_name);

        let mut installed_count = 0;
        let mut missing_count = 0;

        for package in &packages {
            let manager_str = format!("{:?}", package.manager).to_lowercase();

            if is_active {
                // Check installation status for active profile
                let check_result = PackageService::check_package(package);
                let status_str = match check_result.status {
                    PackageCheckStatus::Installed => {
                        installed_count += 1;
                        "\u{2713} installed".to_string()
                    }
                    PackageCheckStatus::NotInstalled => {
                        missing_count += 1;
                        "\u{2717} not installed".to_string()
                    }
                    PackageCheckStatus::Error(ref e) => {
                        format!("? {}", e)
                    }
                    PackageCheckStatus::Unknown => "? unknown".to_string(),
                };

                if verbose {
                    println!("  {}", package.name);
                    println!("    Manager: {}", manager_str);
                    if let Some(ref pkg_name) = package.package_name {
                        println!("    Package: {}", pkg_name);
                    }
                    println!("    Binary: {}", package.binary_name);
                    if let Some(ref desc) = package.description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref cmd) = package.install_command {
                        println!("    Install: {}", cmd);
                    }
                    if let Some(ref check) = package.existence_check {
                        println!("    Check: {}", check);
                    }
                    println!("    Status: {}", status_str);
                    println!();
                } else {
                    println!("  {:<12} {:<8} {}", package.name, manager_str, status_str);
                }
            } else {
                // Non-active profile - no status checks
                if verbose {
                    println!("  {}", package.name);
                    println!("    Manager: {}", manager_str);
                    if let Some(ref pkg_name) = package.package_name {
                        println!("    Package: {}", pkg_name);
                    }
                    println!("    Binary: {}", package.binary_name);
                    if let Some(ref desc) = package.description {
                        println!("    Description: {}", desc);
                    }
                    if let Some(ref cmd) = package.install_command {
                        println!("    Install: {}", cmd);
                    }
                    if let Some(ref check) = package.existence_check {
                        println!("    Check: {}", check);
                    }
                    println!();
                } else {
                    println!("  {:<12} {}", package.name, manager_str);
                }
            }
        }

        // Summary
        if is_active {
            println!(
                "\n{} packages ({} installed, {} missing)",
                packages.len(),
                installed_count,
                missing_count
            );
        } else {
            println!("\n{} packages", packages.len());
        }

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn cmd_packages_add(
        profile: Option<String>,
        name: Option<String>,
        manager: Option<String>,
        binary: Option<String>,
        description: Option<String>,
        package_name: Option<String>,
        install_command: Option<String>,
        existence_check: Option<String>,
    ) -> Result<()> {
        use crate::cli::common::{
            parse_manager, print_error, print_success, prompt_manager, prompt_string,
            prompt_string_optional, CliContext,
        };
        use crate::services::{PackageCreationParams, PackageService};

        let ctx = CliContext::load()?;
        let profile_name = ctx.resolve_profile(profile.as_deref());

        // Validate profile exists
        if !ctx.profile_exists(&profile_name) {
            print_error(&format!("Profile '{}' not found", profile_name));
            std::process::exit(1);
        }

        let is_active = ctx.is_active_profile(&profile_name);

        // Get existing packages to check for duplicates
        let existing = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

        // Prompt for missing required fields
        let name = match name {
            Some(n) => n,
            None => prompt_string("Name", None)?,
        };

        // Check for duplicate
        if existing.iter().any(|p| p.name == name) {
            print_error(&format!(
                "Package '{}' already exists in profile '{}'",
                name, profile_name
            ));
            std::process::exit(1);
        }

        let manager = match manager {
            Some(m) => parse_manager(&m).ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid manager '{}'. Valid: brew, apt, cargo, npm, pip, custom, etc.",
                    m
                )
            })?,
            None => prompt_manager(is_active)?,
        };

        let is_custom = matches!(
            manager,
            crate::utils::profile_manifest::PackageManager::Custom
        );

        let binary = match binary {
            Some(b) => b,
            None => prompt_string("Binary name", None)?,
        };

        // For non-custom, get package_name (defaults to binary)
        let pkg_name = if is_custom {
            String::new()
        } else {
            match package_name {
                Some(p) => p,
                None => prompt_string("Package name in manager", Some(&binary))?,
            }
        };

        // For custom, get install_command (required)
        let install_cmd = if is_custom {
            match install_command {
                Some(c) => c,
                None => prompt_string("Install command", None)?,
            }
        } else {
            String::new()
        };

        // For custom, get existence_check (optional)
        let exist_check = if is_custom {
            match existence_check {
                Some(c) => Some(c),
                None => prompt_string_optional("Existence check")?,
            }
        } else {
            None
        };

        // Description is always optional
        let desc = match description {
            Some(d) => d,
            None => prompt_string_optional("Description")?.unwrap_or_default(),
        };

        // Validate
        let validation = PackageService::validate_package(
            &name,
            &binary,
            is_custom,
            &pkg_name,
            &install_cmd,
            Some(&manager),
        );

        if !validation.is_valid {
            print_error(
                &validation
                    .error_message
                    .unwrap_or_else(|| "Validation failed".to_string()),
            );
            std::process::exit(1);
        }

        // Create package
        let package = PackageService::create_package(PackageCreationParams {
            name: &name,
            description: &desc,
            manager: manager.clone(),
            is_custom,
            package_name: &pkg_name,
            binary_name: &binary,
            install_command: &install_cmd,
            existence_check: exist_check.as_deref().unwrap_or(""),
            manager_check: "",
        });

        // Add to profile
        PackageService::add_package(&ctx.config.repo_path, &profile_name, package)?;

        print_success(&format!(
            "Package '{}' added to profile '{}'",
            name, profile_name
        ));

        Ok(())
    }

    fn cmd_packages_remove(profile: Option<String>, yes: bool, name: Option<String>) -> Result<()> {
        use crate::cli::common::{
            print_error, print_success, prompt_confirm, prompt_select_with_suffix, CliContext,
        };
        use crate::services::PackageService;

        let ctx = CliContext::load()?;
        let profile_name = ctx.resolve_profile(profile.as_deref());

        // Validate profile exists
        if !ctx.profile_exists(&profile_name) {
            print_error(&format!("Profile '{}' not found", profile_name));
            std::process::exit(1);
        }

        let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

        if packages.is_empty() {
            println!("No packages found in profile '{}'", profile_name);
            return Ok(());
        }

        // Find package by name or prompt for selection
        let (index, package_name, manager_str) = match name {
            Some(ref n) => {
                // Find by name
                match packages.iter().position(|p| p.name == *n) {
                    Some(i) => {
                        let mgr = format!("{:?}", packages[i].manager).to_lowercase();
                        (i, n.clone(), mgr)
                    }
                    None => {
                        print_error(&format!(
                            "Package '{}' not found in profile '{}'",
                            n, profile_name
                        ));
                        std::process::exit(1);
                    }
                }
            }
            None => {
                // Interactive selection
                println!(
                    "Select package to remove from profile '{}':\n",
                    profile_name
                );
                let options: Vec<(String, Option<String>)> = packages
                    .iter()
                    .map(|p| {
                        let mgr = format!("{:?}", p.manager).to_lowercase();
                        let suffix = format!("({})", mgr);
                        (p.name.clone(), Some(suffix))
                    })
                    .collect();

                let options_ref: Vec<(&str, Option<&str>)> = options
                    .iter()
                    .map(|(n, s)| (n.as_str(), s.as_deref()))
                    .collect();

                let selected = prompt_select_with_suffix("Package", &options_ref)?;
                let mgr = format!("{:?}", packages[selected].manager).to_lowercase();
                (selected, packages[selected].name.clone(), mgr)
            }
        };

        // Confirm unless --yes
        if !yes {
            let confirm_msg = format!(
                "Remove '{}' ({}) from profile '{}'?",
                package_name, manager_str, profile_name
            );
            if !prompt_confirm(&confirm_msg)? {
                println!("Cancelled.");
                return Ok(());
            }
        }

        // Delete package
        PackageService::delete_package(&ctx.config.repo_path, &profile_name, index)?;

        print_success(&format!(
            "Package '{}' removed from profile '{}'",
            package_name, profile_name
        ));

        Ok(())
    }

    fn cmd_packages_check(profile: Option<String>) -> Result<()> {
        use crate::cli::common::{print_error, print_warning, CliContext};
        use crate::services::{PackageCheckStatus, PackageService};

        let ctx = CliContext::load()?;
        let profile_name = ctx.resolve_profile(profile.as_deref());

        // Validate profile exists
        if !ctx.profile_exists(&profile_name) {
            print_error(&format!("Profile '{}' not found", profile_name));
            std::process::exit(1);
        }

        // Check can only work for active profile
        if !ctx.is_active_profile(&profile_name) {
            print_warning(&format!(
                "Cannot check installation status for non-active profile '{}'",
                profile_name
            ));
            println!("   Packages may be for a different system. Use 'list' to view packages.");
            return Ok(());
        }

        let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

        if packages.is_empty() {
            println!("No packages configured for profile '{}'", profile_name);
            return Ok(());
        }

        println!("Checking packages for profile '{}'...\n", profile_name);

        let mut installed = 0;
        let mut not_installed = 0;
        let mut errors = 0;

        for package in &packages {
            let manager_str = format!("{:?}", package.manager).to_lowercase();
            let result = PackageService::check_package(package);

            let status_str = match result.status {
                PackageCheckStatus::Installed => {
                    installed += 1;
                    "\u{2713} installed"
                }
                PackageCheckStatus::NotInstalled => {
                    not_installed += 1;
                    "\u{2717} not installed"
                }
                PackageCheckStatus::Error(ref e) => {
                    errors += 1;
                    // Print inline for errors
                    println!("  {:<12} {:<8} ? {}", package.name, manager_str, e);
                    continue;
                }
                PackageCheckStatus::Unknown => {
                    errors += 1;
                    "? unknown"
                }
            };

            println!("  {:<12} {:<8} {}", package.name, manager_str, status_str);
        }

        println!();
        if not_installed > 0 {
            println!(
                "{} of {} packages installed ({} missing)",
                installed,
                packages.len(),
                not_installed
            );
        } else {
            println!("{} of {} packages installed", installed, packages.len());
        }

        if errors > 0 {
            println!("({} check errors)", errors);
        }

        Ok(())
    }

    /// Print all available commands with their descriptions (typesafe)
    fn print_all_commands() {
        use clap::CommandFactory;

        let cli = Cli::command();
        let subcommands = cli.get_subcommands();

        for subcmd in subcommands {
            let name = subcmd.get_name();
            let about = subcmd
                .get_about()
                .map(|s| s.to_string())
                .or_else(|| subcmd.get_long_about().map(|s| s.to_string()))
                .unwrap_or_else(|| "No description available".to_string());

            // Format the command name with proper spacing
            let name_width = 15;
            let padded_name = if name.len() <= name_width {
                format!("{:<width$}", name, width = name_width)
            } else {
                name.to_string()
            };

            println!("  {}{}", padded_name, about);

            // Print arguments/flags if any
            for arg in subcmd.get_arguments() {
                if let Some(short) = arg.get_short() {
                    if let Some(long) = arg.get_long() {
                        let help = arg
                            .get_help()
                            .map(|s| s.to_string())
                            .unwrap_or_else(String::new);
                        println!("    -{}, --{:<12} {}", short, long, help);
                    }
                } else if let Some(long) = arg.get_long() {
                    let help = arg
                        .get_help()
                        .map(|s| s.to_string())
                        .unwrap_or_else(String::new);
                    println!("    --{:<15} {}", long, help);
                }
            }
        }
    }
}
