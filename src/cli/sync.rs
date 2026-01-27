//! Sync command for synchronizing with remote repository.

use crate::config::{Config, RepoMode};
use crate::git::GitManager;
use crate::services::ProfileService;
use anyhow::{Context, Result};
use tracing::{info, warn};

/// Execute the sync command.
pub fn execute(message: Option<String>) -> Result<()> {
    info!("CLI: sync command executed");
    let config_path = crate::utils::get_config_path();

    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    // Check if repository is configured (either GitHub or Local mode)
    if !config.is_repo_configured() {
        warn!("CLI sync: Repository not configured");
        eprintln!("❌ Repository not configured. Please run 'dotstate' to set up repository sync.");
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
