//! Storage setup service for GitHub repository setup state machine.
//!
//! This service handles the asynchronous GitHub setup process including:
//! - Token validation
//! - Repository existence check
//! - Repository cloning or creation
//! - Repository initialization
//! - Profile discovery
//!
//! The service uses tokio spawn and oneshot channels to run operations
//! asynchronously while the UI remains responsive.

use crate::config::{Config, GitHubConfig};
use crate::git::GitManager;
use crate::github::GitHubClient;
use crate::ui::{GitHubSetupData, GitHubSetupStep};
use crate::utils::ProfileManifest;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::sync::oneshot;
use tracing::{info, warn};

/// Result of a setup step execution
#[derive(Debug)]
pub enum StepResult {
    /// Step completed successfully, continue to next step
    Continue {
        /// Next step to execute
        next_step: GitHubSetupStep,
        /// Updated setup data
        setup_data: GitHubSetupData,
        /// Status message to display
        status_message: String,
        /// Optional delay before processing next step
        delay_ms: Option<u64>,
    },
    /// Setup completed successfully
    Complete {
        /// Final setup data
        setup_data: GitHubSetupData,
        /// Updated config to save
        github_config: GitHubConfig,
        /// Discovered profiles
        profiles: Vec<String>,
        /// Whether this is a new repository
        is_new_repo: bool,
    },
    /// Step failed with error
    Failed {
        /// Error message to display
        error_message: String,
        /// Whether to clean up local repo
        cleanup_repo: bool,
    },
}

/// Handle for polling step completion
pub struct StepHandle {
    /// Oneshot receiver for the step result
    pub receiver: oneshot::Receiver<Result<StepResult>>,
}

impl StepHandle {
    /// Try to receive the result without blocking
    pub fn try_recv(&mut self) -> Option<Result<StepResult>> {
        match self.receiver.try_recv() {
            Ok(result) => Some(result),
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => {
                Some(Err(anyhow::anyhow!("Step channel closed unexpectedly")))
            }
        }
    }
}

/// Service for managing GitHub storage setup
pub struct StorageSetupService;

impl StorageSetupService {
    /// Start processing a setup step asynchronously
    ///
    /// Returns a `StepHandle` that can be polled for the result.
    pub fn start_step(
        runtime: &tokio::runtime::Runtime,
        step: GitHubSetupStep,
        setup_data: GitHubSetupData,
        config: &Config,
    ) -> StepHandle {
        let (sender, receiver) = oneshot::channel();

        // Clone data needed for the async task
        let repo_path = config.repo_path.clone();
        let default_branch = config.default_branch.clone();
        let active_profile = config.active_profile.clone();
        let setup_data_clone = setup_data.clone();

        runtime.spawn(async move {
            let result = Self::process_step_async(
                step,
                setup_data_clone,
                repo_path,
                default_branch,
                active_profile,
            )
            .await;
            let _ = sender.send(result);
        });

        StepHandle { receiver }
    }

    /// Process a setup step asynchronously
    async fn process_step_async(
        step: GitHubSetupStep,
        setup_data: GitHubSetupData,
        repo_path: PathBuf,
        default_branch: String,
        active_profile: String,
    ) -> Result<StepResult> {
        match step {
            GitHubSetupStep::Connecting => Self::handle_connecting(setup_data).await,
            GitHubSetupStep::ValidatingToken => Self::handle_validating_token(setup_data).await,
            GitHubSetupStep::CheckingRepo => Self::handle_checking_repo(setup_data).await,
            GitHubSetupStep::CloningRepo => Self::handle_cloning_repo(setup_data, &repo_path).await,
            GitHubSetupStep::CreatingRepo => Self::handle_creating_repo(setup_data).await,
            GitHubSetupStep::InitializingRepo => {
                Self::handle_initializing_repo(
                    setup_data,
                    &repo_path,
                    &default_branch,
                    &active_profile,
                )
                .await
            }
            GitHubSetupStep::DiscoveringProfiles => {
                Self::handle_discovering_profiles(setup_data, &repo_path).await
            }
            GitHubSetupStep::Complete => Self::handle_complete(setup_data, &repo_path).await,
        }
    }

    /// Handle the Connecting step (transition to `ValidatingToken`)
    async fn handle_connecting(setup_data: GitHubSetupData) -> Result<StepResult> {
        Ok(StepResult::Continue {
            next_step: GitHubSetupStep::ValidatingToken,
            setup_data,
            status_message: "Validating your token...".to_string(),
            delay_ms: Some(800),
        })
    }

    /// Handle the `ValidatingToken` step
    async fn handle_validating_token(mut setup_data: GitHubSetupData) -> Result<StepResult> {
        let client = GitHubClient::new(setup_data.token.clone());

        match client.get_user().await {
            Ok(user) => {
                let repo_exists = client
                    .repo_exists(&user.login, &setup_data.repo_name)
                    .await?;

                setup_data.username = Some(user.login);
                setup_data.repo_exists = Some(repo_exists);

                Ok(StepResult::Continue {
                    next_step: GitHubSetupStep::CheckingRepo,
                    setup_data,
                    status_message: "Checking if repository exists...".to_string(),
                    delay_ms: Some(600),
                })
            }
            Err(e) => Ok(StepResult::Failed {
                error_message: format!("Authentication failed: {e}"),
                cleanup_repo: false,
            }),
        }
    }

    /// Handle the `CheckingRepo` step
    async fn handle_checking_repo(setup_data: GitHubSetupData) -> Result<StepResult> {
        // Extract and validate required fields using pattern matching
        let (Some(username), Some(_repo_exists)) =
            (setup_data.username.clone(), setup_data.repo_exists)
        else {
            return Ok(StepResult::Failed {
                error_message: "Internal error: Setup state is invalid. Please try again."
                    .to_string(),
                cleanup_repo: false,
            });
        };

        let repo_name = setup_data.repo_name.clone();

        if setup_data.repo_exists == Some(true) {
            Ok(StepResult::Continue {
                next_step: GitHubSetupStep::CloningRepo,
                setup_data,
                status_message: format!("Cloning repository {username}/{repo_name}..."),
                delay_ms: Some(500),
            })
        } else {
            Ok(StepResult::Continue {
                next_step: GitHubSetupStep::CreatingRepo,
                setup_data,
                status_message: format!("Creating repository {username}/{repo_name}..."),
                delay_ms: Some(600),
            })
        }
    }

    /// Handle the `CloningRepo` step
    async fn handle_cloning_repo(
        mut setup_data: GitHubSetupData,
        repo_path: &Path,
    ) -> Result<StepResult> {
        let username = match setup_data.username.as_ref() {
            Some(u) => u.clone(),
            None => {
                return Ok(StepResult::Failed {
                    error_message: "Internal error: Username not available. Please try again."
                        .to_string(),
                    cleanup_repo: false,
                });
            }
        };

        let remote_url = format!(
            "https://github.com/{}/{}.git",
            username, setup_data.repo_name
        );

        // Clone is a blocking operation, run in spawn_blocking
        let repo_path_clone = repo_path.to_path_buf();
        let token = setup_data.token.clone();
        let clone_result = tokio::task::spawn_blocking(move || {
            GitManager::clone_or_open(&remote_url, &repo_path_clone, Some(&token))
        })
        .await?;

        match clone_result {
            Ok((_, was_existing)) => {
                let status = if was_existing {
                    "Using existing repository!"
                } else {
                    "Repository cloned successfully!"
                };

                setup_data.is_new_repo = false; // Cloned repos are not new

                Ok(StepResult::Continue {
                    next_step: GitHubSetupStep::DiscoveringProfiles,
                    setup_data,
                    status_message: status.to_string(),
                    delay_ms: Some(600),
                })
            }
            Err(e) => Ok(StepResult::Failed {
                error_message: format!("Failed to clone repository: {e}"),
                cleanup_repo: true,
            }),
        }
    }

    /// Handle the `CreatingRepo` step
    async fn handle_creating_repo(mut setup_data: GitHubSetupData) -> Result<StepResult> {
        if setup_data.username.is_none() {
            return Ok(StepResult::Failed {
                error_message: "Internal error: Username not available. Please try again."
                    .to_string(),
                cleanup_repo: false,
            });
        }

        let client = GitHubClient::new(setup_data.token.clone());
        let result = client
            .create_repo(
                &setup_data.repo_name,
                "My dotfiles managed by dotstate",
                setup_data.is_private,
            )
            .await;

        match result {
            Ok(_) => {
                setup_data.is_new_repo = true;

                Ok(StepResult::Continue {
                    next_step: GitHubSetupStep::InitializingRepo,
                    setup_data,
                    status_message: "Initializing local repository...".to_string(),
                    delay_ms: Some(500),
                })
            }
            Err(e) => Ok(StepResult::Failed {
                error_message: format!("Failed to create repository: {e}"),
                cleanup_repo: false, // No local repo created yet
            }),
        }
    }

    /// Handle the `InitializingRepo` step
    async fn handle_initializing_repo(
        mut setup_data: GitHubSetupData,
        repo_path: &Path,
        default_branch: &str,
        active_profile: &str,
    ) -> Result<StepResult> {
        let username = match setup_data.username.as_ref() {
            Some(u) => u.clone(),
            None => {
                return Ok(StepResult::Failed {
                    error_message: "Internal error: Username not available. Please try again."
                        .to_string(),
                    cleanup_repo: false,
                });
            }
        };

        let token = setup_data.token.clone();
        let repo_name = setup_data.repo_name.clone();
        let repo_path_clone = repo_path.to_path_buf();
        let default_branch_clone = default_branch.to_string();
        let active_profile_clone = active_profile.to_string();

        // Clone values for use in status message after the blocking call
        let username_for_status = username.clone();
        let repo_name_for_status = repo_name.clone();

        // Run initialization in spawn_blocking since it involves filesystem and git operations
        let init_result = tokio::task::spawn_blocking(move || {
            Self::initialize_repo_blocking(
                &repo_path_clone,
                &username,
                &repo_name,
                &token,
                &default_branch_clone,
                &active_profile_clone,
            )
        })
        .await?;

        match init_result {
            Ok(_default_profile_name) => {
                setup_data.is_new_repo = true;

                Ok(StepResult::Continue {
                    next_step: GitHubSetupStep::DiscoveringProfiles,
                    setup_data,
                    status_message: format!(
                        "Setup complete!\n\nRepository: {username_for_status}/{repo_name_for_status}\nLocal path: {repo_path:?}\n\nPreparing profile selection..."
                    ),
                    delay_ms: Some(2000),
                })
            }
            Err(e) => Ok(StepResult::Failed {
                error_message: format!("{e}"),
                cleanup_repo: true,
            }),
        }
    }

    /// Blocking repository initialization
    fn initialize_repo_blocking(
        repo_path: &Path,
        username: &str,
        repo_name: &str,
        token: &str,
        default_branch: &str,
        active_profile: &str,
    ) -> Result<String> {
        std::fs::create_dir_all(repo_path).context("Failed to create repository directory")?;

        let mut git_mgr = GitManager::open_or_init(repo_path)?;

        // Add remote
        let remote_url = format!("https://{token}@github.com/{username}/{repo_name}.git");
        git_mgr.add_remote("origin", &remote_url)?;

        // Create initial commit
        std::fs::write(
            repo_path.join("README.md"),
            format!("# {repo_name}\n\nDotfiles managed by dotstate"),
        )?;

        // Create profile manifest with default profile
        let default_profile_name = if active_profile.is_empty() {
            "Personal".to_string()
        } else {
            active_profile.to_string()
        };

        let manifest = ProfileManifest {
            common: crate::utils::profile_manifest::CommonSection::default(),
            profiles: vec![crate::utils::profile_manifest::ProfileInfo {
                name: default_profile_name.clone(),
                description: None,
                synced_files: Vec::new(),
                packages: Vec::new(),
            }],
        };
        manifest.save(repo_path)?;

        git_mgr.commit_all("Initial commit")?;

        let current_branch = git_mgr
            .get_current_branch()
            .unwrap_or_else(|| default_branch.to_string());

        // Before pushing, fetch and merge any remote commits
        if let Err(e) = git_mgr.pull("origin", &current_branch, Some(token)) {
            info!(
                "Could not pull from remote (this is normal for new repos): {}",
                e
            );
        } else {
            info!("Successfully pulled from remote before pushing");
        }

        git_mgr
            .push("origin", &current_branch, Some(token))
            .context(
                "Failed to push to remote. Check your token permissions:\n\
                     - Fine-grained tokens: needs 'Contents' set to 'Read and write'\n\
                     - Classic tokens: needs 'repo' scope",
            )?;

        if let Err(e) = git_mgr.set_upstream_tracking("origin", &current_branch) {
            // Non-fatal - log and continue
            warn!("Failed to set upstream tracking: {}", e);
        }

        Ok(default_profile_name)
    }

    /// Handle the `DiscoveringProfiles` step
    async fn handle_discovering_profiles(
        mut setup_data: GitHubSetupData,
        repo_path: &Path,
    ) -> Result<StepResult> {
        let repo_path_clone = repo_path.to_path_buf();

        // Run profile discovery in spawn_blocking
        let discovery_result =
            tokio::task::spawn_blocking(move || Self::discover_profiles_blocking(&repo_path_clone))
                .await?;

        match discovery_result {
            Ok((profiles, is_new_created)) => {
                if is_new_created {
                    setup_data.is_new_repo = true;
                }

                let username = setup_data
                    .username
                    .clone()
                    .unwrap_or_else(|| "user".to_string());

                let status_message = if profiles.is_empty() {
                    format!(
                        "Setup complete!\n\nRepository: {}/{}\nLocal path: {:?}\n\nNo profiles found. You can create one from the main menu.\n\nPreparing main menu...",
                        username, setup_data.repo_name, repo_path
                    )
                } else {
                    format!(
                        "Setup complete!\n\nFound {} profile(s) in the repository.\n\nPreparing profile selection...",
                        profiles.len()
                    )
                };

                Ok(StepResult::Continue {
                    next_step: GitHubSetupStep::Complete,
                    setup_data,
                    status_message,
                    delay_ms: Some(2000),
                })
            }
            Err(e) => Ok(StepResult::Failed {
                error_message: format!("Failed to discover profiles: {e}"),
                cleanup_repo: true,
            }),
        }
    }

    /// Blocking profile discovery
    fn discover_profiles_blocking(repo_path: &Path) -> Result<(Vec<String>, bool)> {
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;
        let mut created_default = false;

        // Backfill synced_files if empty
        for profile_info in &mut manifest.profiles {
            if profile_info.synced_files.is_empty() {
                let profile_dir = repo_path.join(&profile_info.name);
                if profile_dir.exists() && profile_dir.is_dir() {
                    profile_info.synced_files =
                        Self::list_files_in_profile_dir(&profile_dir).unwrap_or_default();
                }
            }
        }

        // Save backfilled manifest
        if let Err(e) = manifest.save(repo_path) {
            warn!("Failed to save manifest: {}", e);
        }

        // If no profiles found, create a default
        if manifest.profiles.is_empty() {
            info!("No profiles found in repository, creating default 'Personal' profile");
            let default_profile = crate::utils::profile_manifest::ProfileInfo {
                name: "Personal".to_string(),
                description: None,
                synced_files: Vec::new(),
                packages: Vec::new(),
            };
            manifest.profiles.push(default_profile);

            // Create the profile directory
            let profile_dir = repo_path.join("Personal");
            if let Err(e) = std::fs::create_dir_all(&profile_dir) {
                warn!("Failed to create profile directory: {}", e);
            }

            // Save the manifest
            if let Err(e) = manifest.save(repo_path) {
                warn!("Failed to save manifest with default profile: {}", e);
            }

            created_default = true;
        }

        let profiles: Vec<String> = manifest.profiles.iter().map(|p| p.name.clone()).collect();
        Ok((profiles, created_default))
    }

    /// List files in a profile directory
    fn list_files_in_profile_dir(profile_dir: &Path) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        if profile_dir.is_dir() {
            for entry in std::fs::read_dir(profile_dir)? {
                let entry = entry?;
                let path = entry.path();
                // List both files and directories at the top level only
                if path.is_file() || path.is_symlink() || path.is_dir() {
                    // Get relative path from profile directory
                    if let Ok(relative) = path.strip_prefix(profile_dir) {
                        // Convert to string, handling the path properly
                        if let Some(relative_str) = relative.to_str() {
                            // Remove leading ./ if present
                            let clean_path =
                                relative_str.strip_prefix("./").unwrap_or(relative_str);
                            entries.push(clean_path.to_string());
                        }
                    }
                }
            }
        }
        Ok(entries)
    }

    /// Handle the Complete step
    async fn handle_complete(setup_data: GitHubSetupData, repo_path: &Path) -> Result<StepResult> {
        let repo_path_clone = repo_path.to_path_buf();

        // Load final profile list
        let profiles_result = tokio::task::spawn_blocking(move || {
            match ProfileManifest::load_or_backfill(&repo_path_clone) {
                Ok(manifest) => manifest
                    .profiles
                    .iter()
                    .map(|p| p.name.clone())
                    .collect::<Vec<_>>(),
                Err(_) => Vec::new(),
            }
        })
        .await?;

        let username = setup_data
            .username
            .clone()
            .unwrap_or_else(|| "user".to_string());
        let github_config = GitHubConfig {
            owner: username,
            repo: setup_data.repo_name.clone(),
            token: Some(setup_data.token.clone()),
        };

        Ok(StepResult::Complete {
            setup_data: setup_data.clone(),
            github_config,
            profiles: profiles_result,
            is_new_repo: setup_data.is_new_repo,
        })
    }

    /// Clean up a failed setup attempt
    ///
    /// This should be called when setup fails to ensure a clean state for retry.
    pub fn cleanup_failed_setup(config: &mut Config, config_path: &Path, cleanup_repo: bool) {
        info!("Cleaning up failed setup state");

        // Clean up repo directory if requested and it exists
        if cleanup_repo && config.repo_path.exists() {
            // Only clean up if it's a dotstate-created repo (has .git)
            if config.repo_path.join(".git").exists() {
                info!("Removing partially created repo at {:?}", config.repo_path);
                if let Err(e) = std::fs::remove_dir_all(&config.repo_path) {
                    warn!("Failed to clean up repo directory: {}", e);
                }
            }
        }

        // Reset config to unconfigured state
        config.reset_to_unconfigured();

        // Save the reset config
        if let Err(e) = config.save(config_path) {
            warn!("Failed to save reset config: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_step_result_variants() {
        // Test that StepResult variants can be constructed
        let continue_result = StepResult::Continue {
            next_step: GitHubSetupStep::ValidatingToken,
            setup_data: GitHubSetupData {
                token: "test".to_string(),
                repo_name: "test-repo".to_string(),
                username: None,
                repo_exists: None,
                is_private: true,
                delay_until: None,
                is_new_repo: false,
            },
            status_message: "Testing...".to_string(),
            delay_ms: Some(500),
        };

        match continue_result {
            StepResult::Continue { next_step, .. } => {
                assert_eq!(next_step, GitHubSetupStep::ValidatingToken);
            }
            _ => panic!("Expected Continue variant"),
        }
    }

    #[test]
    fn test_failed_result() {
        let failed_result = StepResult::Failed {
            error_message: "Test error".to_string(),
            cleanup_repo: true,
        };

        match failed_result {
            StepResult::Failed {
                error_message,
                cleanup_repo,
            } => {
                assert_eq!(error_message, "Test error");
                assert!(cleanup_repo);
            }
            _ => panic!("Expected Failed variant"),
        }
    }
}
