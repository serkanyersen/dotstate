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
    },
    /// Activate the symlinks, restores app state after deactivation.
    Activate,
    /// Deactivate symlinks. this might be useful if you are going to uninstall dotstate or you need the original files.
    Deactivate {
        /// Completely remove symlinks without restoring files
        #[arg(long)]
        completely: bool,
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
}

impl Cli {
    /// Execute the CLI command
    pub fn execute(self) -> Result<()> {
        match self.command {
            Some(Commands::Sync { message }) => Self::cmd_sync(message),
            Some(Commands::List { verbose }) => Self::cmd_list(verbose),
            Some(Commands::Add { path }) => Self::cmd_add(path),
            Some(Commands::Activate) => Self::cmd_activate(),
            Some(Commands::Deactivate { completely }) => Self::cmd_deactivate(completely),
            Some(Commands::Help { command }) => Self::cmd_help(command),
            Some(Commands::Logs) => Self::cmd_logs(),
            Some(Commands::Config) => Self::cmd_config(),
            Some(Commands::Repository) => Self::cmd_repository(),
            None => {
                // No command provided, launch TUI
                Ok(())
            }
        }
    }

    fn cmd_sync(message: Option<String>) -> Result<()> {
        info!("CLI: sync command executed");
        let config_path = crate::utils::get_config_path();

        let config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if config.github.is_none() {
            warn!("CLI sync: GitHub not configured");
            eprintln!("❌ GitHub not configured. Please run 'dotstate' to set up GitHub sync.");
            std::process::exit(1);
        }

        let repo_path = &config.repo_path;
        let git_mgr = GitManager::open_or_init(repo_path).context("Failed to open repository")?;

        let branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| config.default_branch.clone());
        let token_string = config.get_github_token();
        let token = token_string.as_deref();

        if token.is_none() {
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

        println!("📥 Pulling changes from remote (with rebase)...");
        let pulled_count = git_mgr
            .pull_with_rebase("origin", &branch, token)
            .context("Failed to pull from remote")?;

        println!("📤 Pushing to GitHub...");
        git_mgr
            .push("origin", &branch, token)
            .context("Failed to push to remote")?;

        if pulled_count > 0 {
            info!("CLI sync completed: pulled {} commit(s)", pulled_count);
            println!(
                "✅ Successfully synced with remote! Pulled {} change(s) from remote.",
                pulled_count
            );
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
        println!("Logs are being written to: {:?}", log_dir);
        println!("View logs in real-time: tail -f {:?}", log_file);
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

        // Get active profile's synced files from manifest
        let manifest = crate::utils::ProfileManifest::load_or_backfill(&config.repo_path)
            .context("Failed to load profile manifest")?;
        let empty_vec = Vec::new();
        let synced_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == config.active_profile)
            .map(|p| &p.synced_files)
            .unwrap_or(&empty_vec);

        if synced_files.is_empty() {
            println!("No files are currently synced.");
            return Ok(());
        }

        let home_dir = dirs::home_dir().context("Failed to get home directory")?;
        let repo_path = &config.repo_path;
        let profile_name = &config.active_profile;

        println!("Synced files ({}):", synced_files.len());
        for file in synced_files {
            let symlink_path = home_dir.join(file);
            let repo_file_path = repo_path.join(profile_name).join(file);

            if verbose {
                let symlink_exists = symlink_path.exists();
                let repo_file_exists = repo_file_path.exists();

                println!("  {}", file);
                println!("    Symlink:   {}", symlink_path.display());
                if symlink_exists {
                    // Check if it's actually a symlink
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

        Ok(())
    }

    fn cmd_add(path: PathBuf) -> Result<()> {
        use crate::utils::SymlinkManager;

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

        // Sanity checks
        let repo_path = &config.repo_path;
        let (is_safe, reason) = crate::utils::is_safe_to_add(&resolved_path, repo_path);
        if !is_safe {
            eprintln!(
                "❌ {}",
                reason.unwrap_or_else(|| "Cannot add this path".to_string())
            );
            eprintln!("   Path: {:?}", resolved_path);
            std::process::exit(1);
        }

        // Check if it's a git repo (deny if directory is a git repo)
        if resolved_path.is_dir() && crate::utils::is_git_repo(&resolved_path) {
            eprintln!(
                "❌ Cannot sync a git repository. Path contains a .git directory: {:?}",
                resolved_path
            );
            eprintln!("   You cannot have a git repository inside a git repository.");
            std::process::exit(1);
        }

        // Show confirmation prompt
        println!("⚠️  Warning: This will move the following path to the storage repo and replace it with a symlink:");
        println!("   {}", resolved_path.display());
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

        // Get relative path from home
        let relative_path = resolved_path
            .strip_prefix(&home)
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|_| resolved_path.clone());

        let relative_str = relative_path.to_string_lossy().to_string();
        let profile_name = config.active_profile.clone();
        let repo_path = config.repo_path.clone();

        // Check if already synced
        let manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)
            .context("Failed to load profile manifest")?;

        if let Some(profile) = manifest.profiles.iter().find(|p| p.name == profile_name) {
            if profile.synced_files.contains(&relative_str) {
                println!("ℹ️  File is already synced: {}", relative_str);
                return Ok(());
            }
        } else {
            eprintln!("❌ Active profile '{}' not found in manifest", profile_name);
            std::process::exit(1);
        }

        // Copy file to repo
        let file_manager = crate::file_manager::FileManager::new()?;
        let profile_path = repo_path.join(&profile_name);
        let repo_file_path = profile_path.join(&relative_path);

        // Create parent directories
        if let Some(parent) = repo_file_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create repo directory")?;
        }

        // Handle symlinks: resolve to original file
        let source_path = if file_manager.is_symlink(&resolved_path) {
            file_manager
                .resolve_symlink(&resolved_path)
                .context("Failed to resolve symlink")?
        } else {
            resolved_path.clone()
        };

        // Copy to repo
        file_manager
            .copy_to_repo(&source_path, &repo_file_path)
            .context("Failed to copy file to repo")?;

        // Create symlink using SymlinkManager
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(repo_path.clone(), config.backup_enabled)?;
        symlink_mgr
            .activate_profile(&profile_name, std::slice::from_ref(&relative_str))
            .context("Failed to create symlink")?;

        // Update manifest
        let mut manifest = crate::utils::ProfileManifest::load_or_backfill(&repo_path)?;
        let current_files = manifest
            .profiles
            .iter()
            .find(|p| p.name == profile_name)
            .map(|p| p.synced_files.clone())
            .unwrap_or_default();
        if !current_files.contains(&relative_str) {
            let mut new_files = current_files;
            new_files.push(relative_str.clone());
            manifest.update_synced_files(&profile_name, new_files)?;
            manifest.save(&repo_path)?;
        }

        // Check if this is a custom file (not in default dotfile candidates)
        use crate::dotfile_candidates::get_default_dotfile_paths;
        let default_paths = get_default_dotfile_paths();
        let is_custom = !default_paths.iter().any(|p| p == &relative_str);

        if is_custom {
            // Add to config.custom_files if not already there
            let mut config =
                Config::load_or_create(&config_path).context("Failed to load configuration")?;
            if !config.custom_files.contains(&relative_str) {
                config.custom_files.push(relative_str.clone());
                config.save(&config_path)?;
            }
        }

        println!(
            "✅ Added {} to repository and created symlink",
            relative_str
        );
        Ok(())
    }

    fn cmd_activate() -> Result<()> {
        let config_path = crate::utils::get_config_path();
        let mut config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if config.github.is_none() {
            eprintln!("❌ GitHub not configured. Please run 'dotstate' to set up GitHub sync.");
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

        // Activate profile
        let operations =
            symlink_mgr.activate_profile(&active_profile_name, &active_profile_files)?;

        // Report results
        let success_count = operations
            .iter()
            .filter(|op| op.status == crate::utils::symlink_manager::OperationStatus::Success)
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

    fn cmd_deactivate(completely: bool) -> Result<()> {
        let config_path = crate::utils::get_config_path();
        let mut config =
            Config::load_or_create(&config_path).context("Failed to load configuration")?;

        if config.github.is_none() {
            eprintln!("❌ GitHub not configured. Please run 'dotstate' to set up GitHub sync.");
            std::process::exit(1);
        }

        // Get active profile
        let active_profile_name = &config.active_profile;

        if completely {
            println!(
                "🔓 Deactivating profile '{}' (completely)...",
                active_profile_name
            );
            println!("   This will remove symlinks without restoring files");
        } else {
            println!("🔓 Deactivating profile '{}'...", active_profile_name);
            println!("   This will restore original files from the repository");
        }

        // Create SymlinkManager
        let mut symlink_mgr =
            SymlinkManager::new_with_backup(config.repo_path.clone(), config.backup_enabled)?;

        // Check if tracking file exists and has data
        let tracking_file = crate::utils::get_config_dir().join("symlinks.json");
        if !tracking_file.exists() {
            eprintln!(
                "⚠️  Warning: Tracking file not found at {:?}",
                tracking_file
            );
            eprintln!("   No symlinks are currently tracked.");
            eprintln!("   If you have symlinks, they may have been created outside of dotstate.");
            return Ok(());
        }

        // Check what's in the tracking file
        let tracking_data =
            std::fs::read_to_string(&tracking_file).context("Failed to read tracking file")?;
        let tracking: serde_json::Value =
            serde_json::from_str(&tracking_data).context("Failed to parse tracking file")?;

        if let Some(symlinks) = tracking.get("symlinks").and_then(|s| s.as_array()) {
            if symlinks.is_empty() {
                eprintln!("⚠️  Warning: Tracking file exists but contains no symlinks.");
                eprintln!(
                    "   Profile '{}' may not have any active symlinks.",
                    active_profile_name
                );
                return Ok(());
            }

            // Debug: show what profiles are tracked
            let profile_path = config.repo_path.join(active_profile_name);
            let profile_path_str = profile_path.to_string_lossy().to_string();
            let profile_symlinks: Vec<_> = symlinks
                .iter()
                .filter_map(|s| {
                    s.get("source")
                        .and_then(|src| src.as_str())
                        .filter(|src| src.starts_with(&profile_path_str))
                })
                .collect();

            if profile_symlinks.is_empty() {
                eprintln!(
                    "⚠️  Warning: No symlinks found for profile '{}'",
                    active_profile_name
                );
                if let Some(active) = tracking.get("active_profile").and_then(|a| a.as_str()) {
                    if !active.is_empty() && active != active_profile_name {
                        eprintln!(
                            "   Currently tracked active profile: '{}' (different from config)",
                            active
                        );
                        eprintln!(
                            "   Your config has active_profile = '{}'",
                            active_profile_name
                        );
                        eprintln!("   This mismatch might be the issue.");
                    }
                }
                eprintln!("   Profile path expected: {:?}", profile_path);
                eprintln!("   Total symlinks in tracking file: {}", symlinks.len());

                // Show what profiles are actually tracked
                let mut tracked_profiles = std::collections::HashSet::new();
                for symlink in symlinks {
                    if let Some(source) = symlink.get("source").and_then(|s| s.as_str()) {
                        // Extract profile name from source path
                        if let Some(repo_path_str) = config.repo_path.to_str() {
                            if let Some(relative) = source.strip_prefix(repo_path_str) {
                                if let Some(profile_name) = relative.split('/').next() {
                                    if !profile_name.is_empty() && profile_name != "." {
                                        tracked_profiles.insert(profile_name);
                                    }
                                }
                            }
                        }
                    }
                }
                if !tracked_profiles.is_empty() {
                    eprintln!("   Profiles found in tracking file: {:?}", tracked_profiles);
                }

                return Ok(());
            }
        }

        // Deactivate profile
        let operations =
            symlink_mgr.deactivate_profile_with_restore(active_profile_name, !completely)?;

        // Report results
        let success_count = operations
            .iter()
            .filter(|op| op.status == crate::utils::symlink_manager::OperationStatus::Success)
            .count();
        let failed_count = operations.len() - success_count;

        if operations.is_empty() {
            eprintln!(
                "⚠️  No symlinks found to deactivate for profile '{}'",
                active_profile_name
            );
            eprintln!("   This could mean:");
            eprintln!("   - The profile was never activated");
            eprintln!("   - The symlinks were created outside of dotstate");
            eprintln!("   - The profile name doesn't match what's in the tracking file");
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

            if completely {
                println!(
                    "✅ Successfully deactivated profile '{}'",
                    active_profile_name
                );
                println!("   {} symlinks removed", success_count);
            } else {
                println!(
                    "✅ Successfully deactivated profile '{}'",
                    active_profile_name
                );
                println!("   {} files restored", success_count);
            }
            println!("💡 Profile is now deactivated. Use 'dotstate activate' to reactivate.");
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
