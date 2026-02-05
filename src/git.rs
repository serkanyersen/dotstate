use anyhow::{Context, Result};
use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks, Repository, Signature};
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

/// Redact credentials/tokens from a git URL for safe display/logging.
///
/// Handles formats like:
/// - `https://ghp_TOKEN@github.com/user/repo.git` -> `https://***@github.com/user/repo.git`
/// - `https://user:password@github.com/user/repo.git` -> `https://***@github.com/user/repo.git`
/// - `https://oauth2:TOKEN@github.com/user/repo.git` -> `https://***@github.com/user/repo.git`
/// - `https://github.com/user/repo.git` -> unchanged
#[must_use]
pub fn redact_credentials(url: &str) -> String {
    if let Some(protocol_end) = url.find("://") {
        let after_protocol = &url[protocol_end + 3..];
        if let Some(at_pos) = after_protocol.find('@') {
            let host_and_path = &after_protocol[at_pos + 1..];
            return format!("{}://***@{}", &url[..protocol_end], host_and_path);
        }
    }
    url.to_string()
}

/// Remove credentials from a git URL.
///
/// Transforms:
/// - `https://token@github.com/user/repo.git` -> `https://github.com/user/repo.git`
/// - `https://user:pass@github.com/user/repo.git` -> `https://github.com/user/repo.git`
#[must_use]
pub fn remove_credentials_from_url(url: &str) -> String {
    if let Some(protocol_end) = url.find("://") {
        let after_protocol = &url[protocol_end + 3..];
        if let Some(at_pos) = after_protocol.find('@') {
            let host_and_path = &after_protocol[at_pos + 1..];
            return format!("{}://{}", &url[..protocol_end], host_and_path);
        }
    }
    url.to_string()
}

/// Add credentials to a git URL.
///
/// Transforms:
/// - `https://github.com/user/repo.git` + token -> `https://token@github.com/user/repo.git`
///
/// If URL already has credentials, they are replaced.
#[must_use]
pub fn add_credentials_to_url(url: &str, token: &str) -> String {
    // First remove any existing credentials
    let clean_url = remove_credentials_from_url(url);

    // Then add the new token
    if clean_url.starts_with("https://") {
        clean_url.replacen("https://", &format!("https://{token}@"), 1)
    } else {
        clean_url
    }
}

/// Check if a URL has embedded credentials.
#[must_use]
pub fn url_has_credentials(url: &str) -> bool {
    if let Some(protocol_end) = url.find("://") {
        let after_protocol = &url[protocol_end + 3..];
        return after_protocol.contains('@');
    }
    false
}

/// Check if a URL is an SSH-based git URL.
///
/// Returns true for URLs like:
/// - `git@github.com:user/repo.git`
/// - `ssh://git@github.com/user/repo.git`
#[must_use]
pub fn is_ssh_url(url: &str) -> bool {
    url.starts_with("git@") || url.starts_with("ssh://")
}

/// Fetch from remote using system git CLI.
///
/// This is used for SSH URLs where libssh2 (used by git2) has compatibility
/// issues with certain SSH agent implementations (e.g., 1Password, `YubiKey`).
/// System git uses OpenSSH which handles all agent types correctly.
fn fetch_via_cli(repo_path: &Path, remote_name: &str, branch: &str) -> Result<()> {
    info!("Using system git for SSH fetch: {} {}", remote_name, branch);
    let output = Command::new("git")
        .args(["fetch", remote_name, branch])
        .current_dir(repo_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run 'git fetch'. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Failed to fetch from remote '{}': {}",
            remote_name,
            stderr.trim()
        );
    }
    Ok(())
}

/// Push to remote using system git CLI.
///
/// Used for SSH URLs to ensure compatibility with all SSH agent implementations.
fn push_via_cli(repo_path: &Path, remote_name: &str, refspec: &str) -> Result<()> {
    info!("Using system git for SSH push: {} {}", remote_name, refspec);
    let output = Command::new("git")
        .args(["push", remote_name, refspec])
        .current_dir(repo_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run 'git push'. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Failed to push to remote '{}': {}",
            remote_name,
            stderr.trim()
        );
    }
    Ok(())
}

/// Clone a repository using system git CLI.
///
/// Used for SSH URLs to ensure compatibility with all SSH agent implementations.
fn clone_via_cli(url: &str, path: &Path) -> Result<()> {
    info!(
        "Using system git for SSH clone: {}",
        redact_credentials(url)
    );
    let output = Command::new("git")
        .args(["clone", url, &path.to_string_lossy()])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run 'git clone'. Is git installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Failed to clone repository from {}: {}",
            redact_credentials(url),
            stderr.trim()
        );
    }
    Ok(())
}

/// Git operations for managing the dotfiles repository
pub struct GitManager {
    repo: Repository,
}

impl GitManager {
    /// Open or initialize a repository
    pub fn open_or_init(repo_path: &Path) -> Result<Self> {
        let repo = if repo_path.join(".git").exists() {
            Repository::open(repo_path)
                .with_context(|| format!("Failed to open repository: {repo_path:?}"))?
        } else {
            // Initialize as a normal (non-bare) repository so it has a working directory
            let mut repo = Repository::init(repo_path)
                .with_context(|| format!("Failed to initialize repository: {repo_path:?}"))?;

            // Create .gitignore with common patterns for frequently changing files
            Self::ensure_gitignore(repo_path)?;

            // Ensure default branch is "main" (git2 might use system default which could be "master")
            Self::ensure_main_branch(&repo)?;

            // Configure the repository for better defaults
            Self::configure_repo(&mut repo)?;

            repo
        };

        // Ensure .gitignore exists even for existing repos (won't overwrite if it exists)
        let _ = Self::ensure_gitignore(repo_path);

        // Verify the repository has a working directory (not bare)
        if repo.is_bare() {
            return Err(anyhow::anyhow!(
                "Repository at {repo_path:?} is a bare repository and has no working directory. \
                Cannot add files to index."
            ));
        }

        Ok(Self { repo })
    }

    /// Get the working directory of the repository.
    ///
    /// Returns the repo workdir path, or an error if the repo is bare.
    fn repo_workdir(&self) -> Result<&Path> {
        self.repo
            .workdir()
            .ok_or_else(|| anyhow::anyhow!("Repository has no working directory (bare repo)"))
    }

    /// Get the remote URL for a given remote name.
    fn get_remote_url(&self, remote_name: &str) -> Result<String> {
        let remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{remote_name}' not found"))?;
        remote
            .url()
            .map(String::from)
            .ok_or_else(|| anyhow::anyhow!("Remote '{remote_name}' has no URL"))
    }

    /// Ensure .gitignore exists with common patterns for frequently changing files
    fn ensure_gitignore(repo_path: &Path) -> Result<()> {
        use std::fs;
        use std::io::Write;

        let gitignore_path = repo_path.join(".gitignore");

        // If .gitignore already exists, don't overwrite it
        if gitignore_path.exists() {
            return Ok(());
        }

        let mut file = fs::File::create(&gitignore_path)
            .with_context(|| format!("Failed to create .gitignore at {gitignore_path:?}"))?;

        // Write common patterns for files that shouldn't be tracked
        writeln!(file, "# OS files")?;
        writeln!(file, ".DS_Store")?;
        writeln!(file, "Thumbs.db")?;
        writeln!(file)?;
        writeln!(file, "# Backup files")?;
        writeln!(file, "*.bak")?;
        writeln!(file, "*.swp")?;
        writeln!(file, "*.swo")?;
        writeln!(file, "*~")?;

        Ok(())
    }

    /// Ensure the repository uses "main" as the default branch
    /// If the repo was just initialized and has "master", rename it to "main"
    fn ensure_main_branch(repo: &Repository) -> Result<()> {
        // Check if HEAD exists and what branch it points to
        if let Ok(head) = repo.head() {
            if let Some(branch_name) = head.name().and_then(|n| n.strip_prefix("refs/heads/")) {
                if branch_name == "master" {
                    // Rename master to main
                    let master_ref = repo.find_reference("refs/heads/master")?;
                    if let Some(target) = master_ref.target() {
                        repo.reference("refs/heads/main", target, true, "Rename master to main")?;
                        // Update HEAD to point to main
                        repo.set_head("refs/heads/main")?;
                        // Delete old master branch
                        repo.find_reference("refs/heads/master")?.delete()?;
                    }
                }
            }
        } else {
            // No HEAD yet - this is fine, the first commit will create the branch
            // We can't set HEAD to a non-existent branch, so we'll handle it in commit_all
        }
        Ok(())
    }

    /// Configure repository with proper defaults
    fn configure_repo(repo: &mut Repository) -> Result<()> {
        // Set up default branch name to "main" in git config
        let mut config = repo.config().context("Failed to get repository config")?;

        // Set init.defaultBranch to "main" so future operations use main
        config
            .set_str("init.defaultBranch", "main")
            .context("Failed to set init.defaultBranch")?;

        Ok(())
    }

    /// Generate a commit message based on changed files
    pub fn generate_commit_message(&self) -> Result<String> {
        let changed_files = self.get_changed_files()?;

        if changed_files.is_empty() {
            return Ok("Update dotfiles".to_string());
        }

        const MANIFEST_FILE: &str = ".dotstate-profiles.toml";

        // Check if manifest file is in the changes and if it's the only change
        // The manifest file is permanent - it's added on repo creation and never deleted
        // We ignore it unless it's the ONLY file that changed (meaning profile config was updated)
        let (manifest_changes, other_files): (Vec<&str>, Vec<&str>) = changed_files
            .iter()
            .map(std::string::String::as_str)
            .partition(|s| s.contains(MANIFEST_FILE));

        // If only manifest changed (modified), it means profile configuration was updated
        if !manifest_changes.is_empty() && other_files.is_empty() {
            // Check if it's a modification (not add/delete since manifest is permanent)
            if manifest_changes.iter().any(|s| s.starts_with("M ")) {
                return Ok("Update profile configuration".to_string());
            }
        }

        // Count changes by type (excluding manifest file)
        let mut added = 0;
        let mut modified = 0;
        let mut deleted = 0;
        let mut file_names = Vec::new();

        for file in &other_files {
            if file.starts_with("A ") {
                added += 1;
                file_names.push(file.trim_start_matches("A ").to_string());
            } else if file.starts_with("M ") {
                modified += 1;
                file_names.push(file.trim_start_matches("M ").to_string());
            } else if file.starts_with("D ") {
                deleted += 1;
                file_names.push(file.trim_start_matches("D ").to_string());
            }
        }

        // Build commit message
        let mut parts = Vec::new();

        if added > 0 {
            parts.push(format!(
                "Add {} file{}",
                added,
                if added > 1 { "s" } else { "" }
            ));
        }
        if modified > 0 {
            parts.push(format!(
                "Update {} file{}",
                modified,
                if modified > 1 { "s" } else { "" }
            ));
        }
        if deleted > 0 {
            parts.push(format!(
                "Remove {} file{}",
                deleted,
                if deleted > 1 { "s" } else { "" }
            ));
        }

        let mut message = parts.join(", ");

        // Add file list if reasonable number of files (max 5 files in summary)
        if file_names.len() <= 5 && !file_names.is_empty() {
            // Show profile name if present, otherwise just filenames
            let display_files: Vec<String> = file_names
                .iter()
                .take(5)
                .map(|f| {
                    // Extract just the filename (after profile name) for cleaner display
                    f.split('/').next_back().unwrap_or(f).to_string()
                })
                .collect();

            if !display_files.is_empty() {
                message.push_str(&format!(": {}", display_files.join(", ")));
            }
        } else if file_names.len() > 5 {
            // Show count if too many files
            message.push_str(&format!(" ({}+ files changed)", file_names.len()));
        }

        Ok(message)
    }

    /// Add all changes and commit
    pub fn commit_all(&self, message: &str) -> Result<()> {
        use tracing::info;
        info!("Starting commit: {}", message);

        let mut index = self
            .repo
            .index()
            .context("Failed to get repository index")?;

        // Refresh the index to ensure it's up to date
        index.read(true).context("Failed to refresh index")?;

        // Use add_all with "." to add all files (equivalent to "git add .")
        // Skip vim bundles since they are git repos themselves and vimrc will install them
        index
            .add_all(
                ["."],
                git2::IndexAddOption::DEFAULT,
                Some(&mut |path: &Path, _matched_spec: &[u8]| {
                    // Skip vim bundle directories (they are git repos and will be installed by vimrc)
                    let path_str = path.to_string_lossy();
                    if path_str.contains(".vim/bundle/") || path_str.contains(".vim/plugged/") {
                        1 // Skip vim bundles
                    } else {
                        0 // Accept everything else
                    }
                }),
            )
            .context("Failed to add files to index (git add .)")?;

        index.write().context("Failed to write index")?;

        let tree_id = index.write_tree().context("Failed to write tree")?;
        let tree = self
            .repo
            .find_tree(tree_id)
            .context("Failed to find tree")?;

        let signature = Self::get_signature()?;
        let head = self.repo.head();

        let parent_commit = if let Ok(head) = head {
            Some(
                head.peel_to_commit()
                    .context("Failed to peel HEAD to commit")?,
            )
        } else {
            None
        };

        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

        // For the first commit, create it on "main" branch explicitly
        let branch_ref = if parent_commit.is_none() {
            // First commit - create it on "main" branch
            "refs/heads/main"
        } else {
            // Subsequent commits - use HEAD (which should already point to main)
            "HEAD"
        };

        let commit_oid = self
            .repo
            .commit(
                Some(branch_ref),
                &signature,
                &signature,
                message,
                &tree,
                &parents,
            )
            .context("Failed to create commit")?;

        // After first commit, ensure HEAD points to main
        if parent_commit.is_none() {
            // Update HEAD to point to the newly created main branch
            self.repo
                .set_head("refs/heads/main")
                .context("Failed to set HEAD to main branch")?;
            info!("Created initial commit on main branch: {}", commit_oid);
        } else {
            info!("Created commit: {} ({})", commit_oid, message);
        }

        Ok(())
    }

    /// Reset the last commit, keeping changes staged (git reset --soft HEAD~1)
    ///
    /// This is useful when a push is rejected - we can undo the commit while
    /// preserving the staged changes so the user can fix the issue and re-commit.
    pub fn reset_soft_head(&self) -> Result<()> {
        use tracing::info;

        // Get HEAD commit
        let head = self.repo.head().context("Failed to get HEAD reference")?;
        let head_commit = head.peel_to_commit().context("Failed to get HEAD commit")?;

        // Get parent commit (HEAD~1)
        let parent = head_commit
            .parent(0)
            .context("Cannot reset: no parent commit (this is the initial commit)")?;

        // Reset to parent commit with soft mode (keeps changes staged)
        self.repo
            .reset(parent.as_object(), git2::ResetType::Soft, None)
            .context("Failed to reset to parent commit")?;

        info!(
            "Reset HEAD from {} to {} (soft)",
            head_commit.id(),
            parent.id()
        );
        Ok(())
    }

    /// Cleanup after a failed operation: abort any in-progress rebase and checkout the branch
    ///
    /// This ensures the repository is in a clean state after a failed pull/rebase.
    pub fn cleanup_failed_operation(&self, branch: &str) -> Result<()> {
        use tracing::{info, warn};

        // Check if there's a rebase in progress and abort it
        let rebase_merge_dir = self.repo.path().join("rebase-merge");
        let rebase_apply_dir = self.repo.path().join("rebase-apply");

        if rebase_merge_dir.exists() || rebase_apply_dir.exists() {
            info!("Aborting in-progress rebase");
            // Use git2's state to check and handle rebase
            match self.repo.state() {
                git2::RepositoryState::Rebase
                | git2::RepositoryState::RebaseMerge
                | git2::RepositoryState::RebaseInteractive
                | git2::RepositoryState::ApplyMailboxOrRebase => {
                    // Clean up rebase state by removing the directories
                    if rebase_merge_dir.exists() {
                        let _ = std::fs::remove_dir_all(&rebase_merge_dir);
                    }
                    if rebase_apply_dir.exists() {
                        let _ = std::fs::remove_dir_all(&rebase_apply_dir);
                    }
                }
                _ => {}
            }
        }

        // Checkout the specified branch
        let branch_ref = format!("refs/heads/{branch}");
        if let Ok(reference) = self.repo.find_reference(&branch_ref) {
            // Set HEAD to point to the branch
            self.repo
                .set_head(&branch_ref)
                .context("Failed to set HEAD to branch")?;

            // Get the commit the branch points to
            let commit = reference
                .peel_to_commit()
                .context("Failed to get branch commit")?;

            // Reset working directory to match the branch
            self.repo
                .reset(commit.as_object(), git2::ResetType::Hard, None)
                .context("Failed to reset to branch")?;

            info!("Checked out branch: {}", branch);
        } else {
            warn!("Branch '{}' not found, cannot checkout", branch);
        }

        Ok(())
    }

    /// Build error message for push rejection with sideband context
    fn build_push_rejection_error(errors: &[String], sideband: &[String]) -> String {
        let mut full_error = String::from("Push rejected by remote:");
        // Include sideband messages as additional context (hook output)
        if !sideband.is_empty() {
            full_error.push_str("\n\n");
            full_error.push_str(&sideband.join("\n"));
        }
        full_error.push_str("\n\nRef errors:\n");
        full_error.push_str(&errors.join("\n"));
        full_error
    }

    /// Push to remote
    /// If token is provided, it will be used for authentication.
    /// Otherwise, attempts to extract token from remote URL.
    pub fn push(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<()> {
        use std::cell::RefCell;
        use std::rc::Rc;
        use tracing::info;
        info!("Pushing to remote: {} (branch: {})", remote_name, branch);

        let remote_url = self.get_remote_url(remote_name)?;

        // Use system git for SSH URLs (libssh2 has compatibility issues with
        // some SSH agents like 1Password, `YubiKey`, Secretive)
        if is_ssh_url(&remote_url) {
            let repo_path = self.repo_workdir()?;

            // Handle branch that doesn't exist locally
            let branch_ref = format!("refs/heads/{branch}");
            let refspec = if self.repo.find_reference(&branch_ref).is_err() {
                if let Some(current_branch) = self.get_current_branch() {
                    format!("refs/heads/{current_branch}:refs/heads/{branch}")
                } else {
                    anyhow::bail!("No branch '{branch}' exists and no current branch found");
                }
            } else {
                format!("refs/heads/{branch}:refs/heads/{branch}")
            };

            push_via_cli(repo_path, remote_name, &refspec)?;
            info!("Successfully pushed to {}:{}", remote_name, branch);
            return Ok(());
        }

        let mut remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{remote_name}' not found"))?;

        let mut callbacks = RemoteCallbacks::new();
        let token_to_use = token
            .map(std::string::ToString::to_string)
            .or_else(|| Self::extract_token_from_url(&remote_url));
        Self::setup_credentials(&mut callbacks, token_to_use);

        // Capture push errors from server-side hooks/rejections
        // The push_update_reference callback is called for each ref being updated,
        // and if the server rejects it (e.g., due to hooks), the error message is passed here.
        let push_errors: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let push_errors_clone = push_errors.clone();
        callbacks.push_update_reference(move |refname, status| {
            if let Some(error_msg) = status {
                push_errors_clone
                    .borrow_mut()
                    .push(format!("{refname}: {error_msg}"));
            }
            Ok(())
        });

        // Capture sideband progress messages - this is where full hook output is sent
        // Git hooks send their verbose output (e.g., pre-receive hook messages) through
        // the sideband channel, not through the ref update status.
        let sideband_messages: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
        let sideband_clone = sideband_messages.clone();
        callbacks.sideband_progress(move |data| {
            if let Ok(msg) = std::str::from_utf8(data) {
                let trimmed = msg.trim();
                if !trimmed.is_empty() {
                    sideband_clone.borrow_mut().push(trimmed.to_string());
                }
            }
            true // Continue receiving messages
        });

        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        // Check if branch exists locally
        let branch_ref = format!("refs/heads/{branch}");
        if self.repo.find_reference(&branch_ref).is_err() {
            // Branch doesn't exist, try to get current branch
            if let Some(current_branch) = self.get_current_branch() {
                let refspec = format!("refs/heads/{current_branch}:refs/heads/{branch}");
                remote
                    .push(&[&refspec], Some(&mut push_options))
                    .with_context(|| format!("Failed to push to remote '{remote_name}'"))?;

                // Check for server-side rejections
                let errors = push_errors.borrow();
                let sideband = sideband_messages.borrow();
                // Only error if there are actual ref update failures
                // Sideband messages alone are just progress info, not errors
                if !errors.is_empty() {
                    return Err(anyhow::anyhow!(
                        "{}",
                        Self::build_push_rejection_error(&errors, &sideband)
                    ));
                }
                return Ok(());
            }
            return Err(anyhow::anyhow!(
                "No branch '{branch}' exists and no current branch found"
            ));
        }

        let refspec = format!("refs/heads/{branch}:refs/heads/{branch}");
        remote
            .push(&[&refspec], Some(&mut push_options))
            .with_context(|| {
                format!(
                    "Failed to push to remote '{remote_name}' (URL: {}).\n\n\
                    Check token permissions:\n\
                    • Classic tokens (ghp_): needs 'repo' scope\n\
                    • Fine-grained tokens (github_pat_): needs 'Contents' set to 'Read and write'\n\n\
                    Also verify:\n\
                    • Remote branch exists\n\
                    • You have push access to this repository",
                    redact_credentials(&remote_url)
                )
            })?;

        // Check for server-side rejections (e.g., pre-receive hooks, push rules)
        // The push transport may succeed but the server can still reject individual refs
        let errors = push_errors.borrow();
        let sideband = sideband_messages.borrow();
        // Only error if there are actual ref update failures
        // Sideband messages alone are just progress info, not errors
        if !errors.is_empty() {
            return Err(anyhow::anyhow!(
                "{}",
                Self::build_push_rejection_error(&errors, &sideband)
            ));
        }

        info!("Successfully pushed to {}:{}", remote_name, branch);
        Ok(())
    }

    /// Extract token from a GitHub URL (format: <https://token@github.com>/...)
    fn extract_token_from_url(url: &str) -> Option<String> {
        if let Some(at_pos) = url.find('@') {
            if url.starts_with("https://") {
                let start = 8;
                if at_pos > start {
                    let token_part = &url[start..at_pos];
                    if !token_part.is_empty() {
                        return Some(token_part.to_string());
                    }
                }
            }
        }
        None
    }

    /// Setup credential callback for remote operations
    fn setup_credentials(callbacks: &mut RemoteCallbacks, token: Option<String>) {
        if let Some(token_clone) = token {
            callbacks.credentials(move |_url, username_from_url, allowed_types| {
                // Only use userpass if it's allowed
                if !allowed_types.is_user_pass_plaintext() {
                    return Err(git2::Error::from_str(
                        "User/password authentication not allowed",
                    ));
                }
                // Use canonical GitHub token format: "x-access-token" as username
                let username = username_from_url.unwrap_or("x-access-token");
                Cred::userpass_plaintext(username, &token_clone)
            });
        } else {
            callbacks.credentials(move |url, username_from_url, allowed_types| {
                let url_str = url.to_string();
                let username = username_from_url.unwrap_or("git");

                // Try credential helper for HTTPS URLs (only if allowed)
                if url_str.starts_with("https://") && allowed_types.is_user_pass_plaintext() {
                    // Parse URL to extract protocol, host, and path (standard format)
                    let (host, path) = if let Some(after_proto) = url_str.strip_prefix("https://") {
                        if let Some(slash_idx) = after_proto.find('/') {
                            let h = &after_proto[..slash_idx];
                            let p = &after_proto[slash_idx + 1..];
                            (h, p)
                        } else {
                            (after_proto, "")
                        }
                    } else {
                        ("", "")
                    };

                    let credential_input = format!("protocol=https\nhost={host}\npath={path}\n");

                    if let Ok(output) = Command::new("git")
                        .arg("credential")
                        .arg("fill")
                        .stdin(std::process::Stdio::piped())
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .and_then(|mut child| {
                            use std::io::Write;
                            if let Some(mut stdin) = child.stdin.take() {
                                stdin.write_all(credential_input.as_bytes())?;
                                stdin.flush()?;
                            }
                            child.wait_with_output()
                        })
                    {
                        if output.status.success() {
                            let output_str = String::from_utf8_lossy(&output.stdout);
                            let mut parsed_username = None;
                            let mut parsed_password = None;

                            for line in output_str.lines() {
                                if let Some((key, value)) = line.split_once('=') {
                                    match key {
                                        "username" => parsed_username = Some(value.to_string()),
                                        "password" => parsed_password = Some(value.to_string()),
                                        _ => {}
                                    }
                                }
                            }

                            if let (Some(user), Some(pass)) = (parsed_username, parsed_password) {
                                return Cred::userpass_plaintext(&user, &pass);
                            }
                        }
                    }
                }

                // Try SSH agent for SSH URLs (only if allowed)
                if (url_str.starts_with("git@") || url_str.starts_with("ssh://"))
                    && allowed_types.is_ssh_key()
                {
                    if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                        return Ok(cred);
                    }

                    // Optional: Try default SSH key files if agent fails
                    let home = std::env::var("HOME").ok();
                    if let Some(ref home_dir) = home {
                        let key_paths = [
                            format!("{home_dir}/.ssh/id_ed25519"),
                            format!("{home_dir}/.ssh/id_rsa"),
                        ];

                        for key_path in &key_paths {
                            let key_path_obj = std::path::Path::new(key_path);
                            if key_path_obj.exists() {
                                let pubkey_path_str = format!("{key_path}.pub");
                                let pubkey_path_obj = std::path::Path::new(&pubkey_path_str);
                                // Try without passphrase first (most common case)
                                if pubkey_path_obj.exists() {
                                    if let Ok(cred) = Cred::ssh_key(username, Some(pubkey_path_obj), key_path_obj, None) {
                                        return Ok(cred);
                                    }
                                }
                            }
                        }
                    }
                }

                // Try username-only if allowed
                if allowed_types.is_username() {
                    if let Ok(cred) = Cred::username(username) {
                        return Ok(cred);
                    }
                }

                Err(git2::Error::from_str(
                    "No credentials available. Please ensure you have:\n  - Git credential helper configured (e.g., osxkeychain)\n  - SSH keys set up and added to ssh-agent (for SSH URLs)"
                ))
            });
        }
    }

    /// Pull from remote
    pub fn pull(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<()> {
        use tracing::info;
        info!("Pulling from remote: {} (branch: {})", remote_name, branch);

        let remote_url = self.get_remote_url(remote_name)?;

        // Fetch step: use system git for SSH URLs, git2 for HTTPS
        if is_ssh_url(&remote_url) {
            fetch_via_cli(self.repo_workdir()?, remote_name, branch)?;
        } else {
            let mut remote = self
                .repo
                .find_remote(remote_name)
                .with_context(|| format!("Remote '{remote_name}' not found"))?;

            let mut callbacks = RemoteCallbacks::new();
            let token_to_use = token
                .map(std::string::ToString::to_string)
                .or_else(|| Self::extract_token_from_url(&remote_url));
            Self::setup_credentials(&mut callbacks, token_to_use);

            let mut fetch_options = FetchOptions::new();
            fetch_options.remote_callbacks(callbacks);

            remote
                .fetch(&[branch], Some(&mut fetch_options), None)
                .with_context(|| format!("Failed to fetch from remote '{remote_name}'"))?;
        }

        // Check if FETCH_HEAD exists (remote might not have the branch yet)
        let fetch_head = match self.repo.find_reference("FETCH_HEAD") {
            Ok(ref_) => ref_,
            Err(_) => {
                // No remote commits yet, nothing to merge
                return Ok(());
            }
        };

        let fetch_commit = fetch_head
            .peel_to_commit()
            .context("Failed to peel FETCH_HEAD to commit")?;

        // Check if we have any local commits
        let local_head = match self.repo.head() {
            Ok(head) => head.peel_to_commit().ok(),
            Err(_) => None,
        };

        // If we have local commits and remote commits, we need to merge
        if let Some(local_commit) = local_head {
            // Check if remote is ahead (different commits)
            if local_commit.id() != fetch_commit.id() {
                // Convert commit to annotated commit for merge
                let annotated_commit = self
                    .repo
                    .find_annotated_commit(fetch_commit.id())
                    .context("Failed to create annotated commit")?;

                // Perform the merge
                self.repo
                    .merge(&[&annotated_commit], None, None)
                    .context("Failed to merge")?;

                // Get the index after merge
                let mut index = self
                    .repo
                    .index()
                    .context("Failed to get index after merge")?;

                // Check if merge resulted in conflicts
                if index.has_conflicts() {
                    return Err(anyhow::anyhow!(
                        "Merge conflicts detected. Please resolve manually."
                    ));
                }

                // Write the index after merge
                index.write().context("Failed to write index after merge")?;

                // Create merge commit
                let tree_id = index
                    .write_tree()
                    .context("Failed to write tree after merge")?;
                let tree = self
                    .repo
                    .find_tree(tree_id)
                    .context("Failed to find tree after merge")?;

                // Get signature for commit
                let signature = Self::get_signature()?;

                // Create merge commit with both parents
                self.repo
                    .commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        "Merge remote-tracking branch",
                        &tree,
                        &[&local_commit, &fetch_commit],
                    )
                    .context("Failed to commit merge")?;

                // Clean up merge state
                self.repo
                    .cleanup_state()
                    .context("Failed to cleanup merge state")?;
            }
        } else {
            // No local commits, just update HEAD to point to remote
            let branch_ref = format!("refs/heads/{branch}");
            self.repo.reference(
                &branch_ref,
                fetch_commit.id(),
                true,
                "Update branch from remote",
            )?;
            self.repo.set_head(&branch_ref)?;
            self.repo
                .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
        }

        Ok(())
    }

    /// Pull changes from remote with rebase (instead of merge)
    /// Returns the number of commits pulled from remote
    pub fn pull_with_rebase(
        &self,
        remote_name: &str,
        branch: &str,
        token: Option<&str>,
    ) -> Result<usize> {
        info!(
            "Pulling with rebase from remote: {} (branch: {})",
            remote_name, branch
        );

        let remote_url = self.get_remote_url(remote_name)?;

        // Fetch step: use system git for SSH URLs, git2 for HTTPS
        if is_ssh_url(&remote_url) {
            fetch_via_cli(self.repo_workdir()?, remote_name, branch)?;
        } else {
            let mut remote = self
                .repo
                .find_remote(remote_name)
                .with_context(|| format!("Remote '{remote_name}' not found"))?;

            let mut callbacks = RemoteCallbacks::new();
            let token_to_use = token
                .map(std::string::ToString::to_string)
                .or_else(|| Self::extract_token_from_url(&remote_url));
            Self::setup_credentials(&mut callbacks, token_to_use);

            let mut fetch_options = FetchOptions::new();
            fetch_options.remote_callbacks(callbacks);

            remote
                .fetch(&[branch], Some(&mut fetch_options), None)
                .with_context(|| format!("Failed to fetch from remote '{remote_name}'"))?;
        }

        // Check if FETCH_HEAD exists (remote might not have the branch yet)
        let fetch_head = if let Ok(ref_) = self.repo.find_reference("FETCH_HEAD") {
            ref_
        } else {
            // No remote commits yet, nothing to rebase
            debug!("No remote commits found, nothing to pull");
            return Ok(0);
        };

        let fetch_commit = fetch_head
            .peel_to_commit()
            .context("Failed to peel FETCH_HEAD to commit")?;

        // Check if we have any local commits
        let local_head = match self.repo.head() {
            Ok(head) => head.peel_to_commit().ok(),
            Err(_) => None,
        };

        let fetch_commit_id = fetch_commit.id();

        if let Some(local_commit) = local_head {
            // Check if remote is ahead (different commits)
            if local_commit.id() == fetch_commit_id {
                // Already up to date
                debug!("Already up to date with remote");
                return Ok(0);
            }

            // Find merge base between local and remote
            let merge_base = self
                .repo
                .merge_base(local_commit.id(), fetch_commit_id)
                .context("Failed to find merge base")?;

            // Count commits from merge base to remote HEAD (commits we're pulling)
            let mut pulled_count = 0;
            let mut commit = fetch_commit.clone();
            loop {
                if commit.id() == merge_base {
                    break;
                }
                pulled_count += 1;
                if commit.parent_count() == 0 {
                    break;
                }
                commit = commit.parent(0)?;
            }

            // Check if local is ahead of merge base (we have local commits to rebase)
            let local_ahead = merge_base != local_commit.id();

            if !local_ahead {
                // Local is at merge base, we can fast-forward
                debug!("Fast-forwarding to remote HEAD");
                let branch_ref = format!("refs/heads/{branch}");
                self.repo.reference(
                    &branch_ref,
                    fetch_commit_id,
                    true,
                    "Fast-forward to remote",
                )?;
                self.repo
                    .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
                return Ok(pulled_count);
            }

            // We have local commits that need to be rebased on top of remote
            info!(
                "Rebasing local commits onto remote (merge_base: {}, local: {}, remote: {})",
                merge_base,
                local_commit.id(),
                fetch_commit_id
            );

            // Create annotated commits for rebase
            let upstream_annotated = self
                .repo
                .find_annotated_commit(fetch_commit_id)
                .context("Failed to create annotated commit for upstream")?;

            let branch_annotated = self
                .repo
                .find_annotated_commit(local_commit.id())
                .context("Failed to create annotated commit for branch")?;

            // Start the rebase: rebase local commits onto upstream (remote)
            // branch = our local commits, upstream = remote HEAD, onto = None (use upstream)
            let mut rebase = self
                .repo
                .rebase(
                    Some(&branch_annotated),
                    Some(&upstream_annotated),
                    None,
                    None,
                )
                .context("Failed to initialize rebase")?;

            let signature = Self::get_signature()?;

            // Process each rebase operation
            loop {
                match rebase.next() {
                    Some(Ok(operation)) => {
                        debug!("Rebase operation: {:?}", operation.kind());

                        // Check for conflicts
                        let index = self.repo.index().context("Failed to get index")?;
                        if index.has_conflicts() {
                            // Abort the rebase on conflict
                            let _ = rebase.abort();
                            return Err(anyhow::anyhow!(
                                "Rebase conflicts detected. Please resolve manually:\n\
                                1. Run 'git status' to see conflicted files\n\
                                2. Edit files to resolve conflicts\n\
                                3. Run 'git add <file>' for each resolved file\n\
                                4. Run 'git rebase --continue'"
                            ));
                        }

                        // Commit the rebased change
                        match rebase.commit(None, &signature, None) {
                            Ok(_oid) => {
                                debug!("Rebased commit successfully");
                            }
                            Err(e) => {
                                // If commit fails because there's nothing to commit (empty commit),
                                // that's okay - just continue
                                if e.code() == git2::ErrorCode::Applied {
                                    debug!("Skipping already applied commit");
                                    continue;
                                }
                                // For other errors, abort and return
                                let _ = rebase.abort();
                                return Err(anyhow::anyhow!("Failed to commit during rebase: {e}"));
                            }
                        }
                    }
                    Some(Err(e)) => {
                        let _ = rebase.abort();
                        return Err(anyhow::anyhow!("Rebase operation failed: {e}"));
                    }
                    None => {
                        // No more operations, finish the rebase
                        break;
                    }
                }
            }

            // Finish the rebase
            rebase
                .finish(Some(&signature))
                .context("Failed to finish rebase")?;

            // After rebase, we need to explicitly update the branch reference
            // to point to the new HEAD (which now contains the rebased commits)
            let head = self
                .repo
                .head()
                .context("Failed to get HEAD after rebase")?;
            let head_commit = head
                .peel_to_commit()
                .context("Failed to peel HEAD to commit after rebase")?;

            let branch_ref = format!("refs/heads/{branch}");
            self.repo.reference(
                &branch_ref,
                head_commit.id(),
                true,
                "Update branch after rebase",
            )?;

            // Set HEAD to point to the branch (not detached)
            self.repo.set_head(&branch_ref)?;

            // Make sure working directory is up to date
            self.repo
                .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

            info!(
                "Rebase completed successfully, pulled {} commit(s), HEAD now at {}",
                pulled_count,
                head_commit.id()
            );
            Ok(pulled_count)
        } else {
            // No local commits, just update HEAD to point to remote
            let branch_ref = format!("refs/heads/{branch}");
            self.repo.reference(
                &branch_ref,
                fetch_commit_id,
                true,
                "Update branch from remote",
            )?;
            self.repo.set_head(&branch_ref)?;
            self.repo
                .checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

            // Count all commits in remote
            let mut pulled_count = 0;
            let mut commit = fetch_commit.clone();
            loop {
                pulled_count += 1;
                if commit.parent_count() == 0 {
                    break;
                }
                commit = commit.parent(0)?;
            }
            Ok(pulled_count)
        }
    }

    /// Fetch from remote (without merging)
    pub fn fetch(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<()> {
        use tracing::debug;
        debug!("Fetching from remote: {} (branch: {})", remote_name, branch);

        let remote_url = self.get_remote_url(remote_name)?;

        // Use system git for SSH URLs (libssh2 has compatibility issues with
        // some SSH agents like 1Password, `YubiKey`, Secretive)
        if is_ssh_url(&remote_url) {
            return fetch_via_cli(self.repo_workdir()?, remote_name, branch);
        }

        let mut remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{remote_name}' not found"))?;

        let mut callbacks = RemoteCallbacks::new();
        let token_to_use = token
            .map(std::string::ToString::to_string)
            .or_else(|| Self::extract_token_from_url(&remote_url));
        Self::setup_credentials(&mut callbacks, token_to_use);

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        remote
            .fetch(&[branch], Some(&mut fetch_options), None)
            .with_context(|| format!("Failed to fetch from remote '{remote_name}'"))?;

        Ok(())
    }

    /// Get ahead/behind counts for a branch relative to its upstream
    /// Returns (ahead, behind) tuple
    pub fn get_ahead_behind(&self, remote_name: &str, branch: &str) -> Result<(usize, usize)> {
        let local_ref_name = format!("refs/heads/{branch}");

        // Ensure we have the local branch references
        let local_oid = match self.repo.refname_to_id(&local_ref_name) {
            Ok(oid) => oid,
            Err(_) => return Ok((0, 0)), // Local branch doesn't exist yet
        };

        // For remote, we look for FETCH_HEAD since we just fetched,
        // or try to find the remote tracking branch via standard naming
        let remote_oid = if let Ok(fetch_head) = self.repo.find_reference("FETCH_HEAD") {
            fetch_head.peel_to_commit()?.id()
        } else {
            // Fallback to finding the remote tracking branch ref
            // Note: This might be stale if we didn't just fetch
            let remote_ref_name = format!("refs/remotes/{remote_name}/{branch}");
            match self.repo.refname_to_id(&remote_ref_name) {
                Ok(oid) => oid,
                Err(_) => return Ok((0, 0)), // Remote branch doesn't exist
            }
        };

        let (ahead, behind) = self.repo.graph_ahead_behind(local_oid, remote_oid)?;
        Ok((ahead, behind))
    }

    /// Add a remote (or update if it exists)
    pub fn add_remote(&mut self, name: &str, url: &str) -> Result<()> {
        // remote_set_url doesn't exist in git2, so we delete and recreate
        if self.repo.find_remote(name).is_ok() {
            self.repo
                .remote_delete(name)
                .with_context(|| format!("Failed to delete existing remote '{name}'"))?;
        }
        self.repo
            .remote(name, url)
            .with_context(|| format!("Failed to add remote '{name}'"))?;

        // Configure remote tracking for the current branch
        self.configure_remote_tracking(name)?;

        Ok(())
    }

    /// Update the remote URL to either embed or remove credentials based on setting.
    ///
    /// # Arguments
    /// * `remote_name` - Name of the remote (usually "origin")
    /// * `token` - The token to embed (if `embed_credentials` is true)
    /// * `embed_credentials` - Whether to embed credentials in the URL
    pub fn update_remote_credentials(
        &mut self,
        remote_name: &str,
        token: Option<&str>,
        embed_credentials: bool,
    ) -> Result<()> {
        let remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{remote_name}' not found"))?;

        let current_url = remote
            .url()
            .ok_or_else(|| anyhow::anyhow!("Remote '{remote_name}' has no URL"))?;

        let new_url = if embed_credentials {
            if let Some(token) = token {
                add_credentials_to_url(current_url, token)
            } else {
                // No token available, can't embed
                return Ok(());
            }
        } else {
            remove_credentials_from_url(current_url)
        };

        // Only update if URL actually changed
        if new_url == current_url {
            return Ok(());
        }

        // Delete and recreate remote with new URL
        self.repo
            .remote_delete(remote_name)
            .with_context(|| format!("Failed to delete remote '{remote_name}'"))?;

        self.repo
            .remote(remote_name, &new_url)
            .with_context(|| format!("Failed to recreate remote '{remote_name}' with new URL"))?;

        // Reconfigure tracking
        self.configure_remote_tracking(remote_name)?;

        info!(
            "Updated remote URL: embed_credentials={}",
            embed_credentials
        );
        Ok(())
    }

    /// Update the token embedded in a remote URL
    ///
    /// This is useful when the user updates their GitHub token - the remote URL
    /// needs to be updated to use the new token for authentication.
    ///
    /// # Arguments
    /// * `remote_name` - Name of the remote (usually "origin")
    /// * `new_token` - The new token to embed in the URL
    pub fn update_remote_token(&mut self, remote_name: &str, new_token: &str) -> Result<()> {
        let remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{remote_name}' not found"))?;

        let current_url = remote
            .url()
            .ok_or_else(|| anyhow::anyhow!("Remote '{remote_name}' has no URL"))?;

        // Build new URL with updated token
        let new_url = if current_url.starts_with("https://") {
            // Check if there's already a token in the URL
            if let Some(at_pos) = current_url.find('@') {
                // Replace existing token: https://old_token@github.com/... -> https://new_token@github.com/...
                let host_and_path = &current_url[at_pos + 1..];
                format!("https://{new_token}@{host_and_path}")
            } else {
                // No token in URL, insert after https://
                current_url.replacen("https://", &format!("https://{new_token}@"), 1)
            }
        } else {
            // Not HTTPS, can't embed token
            return Err(anyhow::anyhow!(
                "Cannot update token for non-HTTPS remote URL: {current_url}"
            ));
        };

        // Delete and recreate remote with new URL
        self.repo
            .remote_delete(remote_name)
            .with_context(|| format!("Failed to delete remote '{remote_name}'"))?;

        self.repo
            .remote(remote_name, &new_url)
            .with_context(|| format!("Failed to recreate remote '{remote_name}' with new URL"))?;

        // Reconfigure tracking
        self.configure_remote_tracking(remote_name)?;

        Ok(())
    }

    /// Configure remote tracking for the current branch
    fn configure_remote_tracking(&self, remote_name: &str) -> Result<()> {
        // Get current branch (should be main)
        if let Some(branch_name) = self.get_current_branch() {
            // Set up tracking via git config
            // Format: branch.<name>.remote = <remote>
            // Format: branch.<name>.merge = refs/heads/<name>
            let mut config = self
                .repo
                .config()
                .context("Failed to get repository config")?;

            let remote_key = format!("branch.{branch_name}.remote");
            let merge_key = format!("branch.{branch_name}.merge");

            config
                .set_str(&remote_key, remote_name)
                .context("Failed to set branch remote")?;
            config
                .set_str(&merge_key, &format!("refs/heads/{branch_name}"))
                .context("Failed to set branch merge")?;
        }
        Ok(())
    }

    /// Set upstream tracking for a branch (public method for use after push)
    pub fn set_upstream_tracking(&self, remote_name: &str, branch_name: &str) -> Result<()> {
        // Set up tracking via git config
        let mut config = self
            .repo
            .config()
            .context("Failed to get repository config")?;

        let remote_key = format!("branch.{branch_name}.remote");
        let merge_key = format!("branch.{branch_name}.merge");

        config
            .set_str(&remote_key, remote_name)
            .context("Failed to set branch remote")?;
        config
            .set_str(&merge_key, &format!("refs/heads/{branch_name}"))
            .context("Failed to set branch merge")?;

        Ok(())
    }

    /// Get signature for commits
    fn get_signature() -> Result<Signature<'static>> {
        // Try to get from git config, fallback to defaults
        let config = git2::Config::open_default().ok();

        let name = config
            .as_ref()
            .and_then(|c| c.get_string("user.name").ok())
            .unwrap_or_else(|| "dotstate".to_string());

        let email = config
            .as_ref()
            .and_then(|c| c.get_string("user.email").ok())
            .unwrap_or_else(|| "dotstate@localhost".to_string());

        Ok(Signature::now(&name, &email)?)
    }

    /// Get the repository reference
    #[allow(dead_code)]
    #[must_use]
    pub fn repo(&self) -> &Repository {
        &self.repo
    }

    /// Check if there are uncommitted changes
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let mut index = self
            .repo
            .index()
            .context("Failed to get repository index")?;

        // Refresh the index to get current state
        index.read(true).context("Failed to read index")?;

        // Check if index differs from HEAD
        let head = match self.repo.head() {
            Ok(head) => Some(head.peel_to_tree().context("Failed to peel HEAD to tree")?),
            Err(_) => None,
        };

        if let Some(head_tree) = head {
            let diff = self
                .repo
                .diff_tree_to_index(Some(&head_tree), Some(&index), None)
                .context("Failed to create diff")?;

            // Check if there are any differences
            let has_changes = diff.deltas().next().is_some();

            // Also check for untracked files
            let mut status_opts = git2::StatusOptions::new();
            status_opts.include_untracked(true);
            status_opts.include_ignored(false);

            let statuses = self
                .repo
                .statuses(Some(&mut status_opts))
                .context("Failed to get status")?;

            // Check for any working tree changes (new, modified, deleted, renamed, etc.)
            let has_working_changes = statuses.iter().any(|s| {
                let status = s.status();
                status.intersects(
                    git2::Status::WT_NEW
                        | git2::Status::WT_MODIFIED
                        | git2::Status::WT_DELETED
                        | git2::Status::WT_RENAMED
                        | git2::Status::WT_TYPECHANGE,
                )
            });

            Ok(has_changes || has_working_changes)
        } else {
            // No HEAD, check if index has any entries
            Ok(!index.is_empty())
        }
    }

    /// Check if there are unpushed commits
    pub fn has_unpushed_commits(&self, remote_name: &str, branch: &str) -> Result<bool> {
        // Check if remote exists
        let mut remote = match self.repo.find_remote(remote_name) {
            Ok(r) => r,
            Err(_) => return Ok(false), // No remote, so no unpushed commits
        };

        // Get local branch
        let branch_ref = format!("refs/heads/{branch}");
        let local_branch = match self.repo.find_reference(&branch_ref) {
            Ok(r) => r,
            Err(_) => return Ok(false), // No local branch
        };

        let local_oid = local_branch
            .target()
            .context("Failed to get local branch OID")?;

        // Fetch from remote to update remote refs
        let mut remote_callbacks = RemoteCallbacks::new();
        remote_callbacks.credentials(|_url, _username_from_url, _allowed_types| {
            // For now, we'll just fail if credentials are needed
            // In the future, we could use stored credentials
            Err(git2::Error::from_str("Credentials required"))
        });

        let mut fetch_opts = FetchOptions::new();
        fetch_opts.remote_callbacks(remote_callbacks);

        // Try to fetch (ignore errors - remote might not be accessible)
        let _ = remote.fetch(&[branch], Some(&mut fetch_opts), None);

        // Get remote branch reference
        let remote_ref = format!("refs/remotes/{remote_name}/{branch}");
        let remote_branch = match self.repo.find_reference(&remote_ref) {
            Ok(r) => r,
            Err(_) => return Ok(true), // No remote branch, so we have unpushed commits
        };

        let remote_oid = remote_branch
            .target()
            .context("Failed to get remote branch OID")?;

        // Check if local is ahead of remote (local commit is reachable from remote)
        match self.repo.graph_ahead_behind(local_oid, remote_oid) {
            Ok((ahead, _behind)) => Ok(ahead > 0),
            Err(_) => Ok(true), // Can't determine, assume there are unpushed commits
        }
    }

    /// Get the current branch name
    #[must_use]
    pub fn get_current_branch(&self) -> Option<String> {
        let head = self.repo.head().ok()?;
        let name = head.name()?;
        // Remove 'refs/heads/' prefix
        name.strip_prefix("refs/heads/")
            .map(std::string::ToString::to_string)
    }

    /// Get list of changed files (modified, added, deleted)
    pub fn get_changed_files(&self) -> Result<Vec<String>> {
        let mut status_opts = git2::StatusOptions::new();
        status_opts.include_untracked(true);
        // Show all untracked files including those in subdirectories (equivalent to -uall)
        // This is done by setting recurse_untracked_dirs to true
        status_opts.recurse_untracked_dirs(true);
        status_opts.include_ignored(false);
        status_opts.include_unmodified(false);

        let statuses = self
            .repo
            .statuses(Some(&mut status_opts))
            .context("Failed to get repository status")?;

        let mut changed_files = Vec::new();
        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let status = entry.status();
                let prefix = if status.contains(git2::Status::WT_NEW) {
                    "A " // Added
                } else if status.contains(git2::Status::WT_MODIFIED) {
                    "M " // Modified
                } else if status.contains(git2::Status::WT_DELETED) {
                    "D " // Deleted
                } else if status.contains(git2::Status::INDEX_NEW) {
                    "A " // Staged new
                } else if status.contains(git2::Status::INDEX_MODIFIED) {
                    "M " // Staged modified
                } else if status.contains(git2::Status::INDEX_DELETED) {
                    "D " // Staged deleted
                } else {
                    "? " // Unknown
                };
                changed_files.push(format!("{prefix}{path}"));
            }
        }

        Ok(changed_files)
    }

    /// Clone a repository from a remote URL, or reuse existing repository if valid.
    ///
    /// This is the preferred method for setting up a repository. It:
    /// 1. Checks if a valid repository already exists at the path
    /// 2. Validates the remote URL matches (if `expected_remote_url` is provided)
    /// 3. Either reuses the existing repo or clones fresh
    ///
    /// Returns `Ok((GitManager, was_existing))` where `was_existing` indicates if an
    /// existing repository was reused.
    ///
    /// # Arguments
    /// * `url` - The remote URL to clone from (without token)
    /// * `path` - Local path for the repository
    /// * `token` - Optional GitHub token for authentication
    pub fn clone_or_open(url: &str, path: &Path, token: Option<&str>) -> Result<(Self, bool)> {
        Self::clone_or_open_with_options(url, path, token, true)
    }

    /// Clone or open a repository with explicit control over credential embedding.
    ///
    /// # Arguments
    /// * `url` - The remote URL to clone from
    /// * `path` - Local path for the repository
    /// * `token` - Optional GitHub token for authentication
    /// * `embed_credentials` - Whether to embed credentials in the URL
    pub fn clone_or_open_with_options(
        url: &str,
        path: &Path,
        token: Option<&str>,
        embed_credentials: bool,
    ) -> Result<(Self, bool)> {
        // Check if repository already exists
        if path.join(".git").exists() {
            debug!(
                "Repository already exists at {:?}, attempting to open",
                path
            );

            match Self::open_or_init(path) {
                Ok(git_manager) => {
                    // Validate remote URL matches (comparing without credentials)
                    if let Err(e) = git_manager.validate_remote_url("origin", url) {
                        info!(
                            "Existing repo remote mismatch: {}. Will remove and clone fresh.",
                            e
                        );
                        // Remove and clone fresh
                        std::fs::remove_dir_all(path)
                            .with_context(|| format!("Failed to remove directory {path:?}"))?;
                        let manager =
                            Self::clone_with_options(url, path, token, embed_credentials)?;
                        return Ok((manager, false));
                    }

                    info!("Using existing repository at {:?}", path);
                    return Ok((git_manager, true));
                }
                Err(e) => {
                    info!(
                        "Failed to open existing repo at {:?}: {}. Will remove and clone fresh.",
                        path, e
                    );
                    // Not a valid repo, remove it
                    std::fs::remove_dir_all(path)
                        .with_context(|| format!("Failed to remove directory {path:?}"))?;
                }
            }
        } else if path.exists() {
            // Directory exists but not a git repo, remove it
            info!(
                "Directory exists at {:?} but is not a git repo. Removing.",
                path
            );
            std::fs::remove_dir_all(path)
                .with_context(|| format!("Failed to remove directory {path:?}"))?;
        }

        // Clone fresh
        let manager = Self::clone_with_options(url, path, token, embed_credentials)?;
        Ok((manager, false))
    }

    /// Validate that the repository's remote URL matches the expected URL.
    ///
    /// This compares the normalized URLs (without tokens and trailing .git).
    pub fn validate_remote_url(&self, remote_name: &str, expected_url: &str) -> Result<()> {
        let remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{remote_name}' not found"))?;

        let actual_url = remote
            .url()
            .ok_or_else(|| anyhow::anyhow!("Remote '{remote_name}' has no URL"))?;

        // Normalize URLs for comparison (remove token, trailing .git)
        let normalize = |url: &str| -> String {
            let mut normalized = url.to_lowercase();

            // Remove token from URL (https://token@github.com -> https://github.com)
            if let Some(at_pos) = normalized.find('@') {
                if normalized.starts_with("https://") {
                    normalized = format!("https://{}", &normalized[at_pos + 1..]);
                }
            }

            // Remove trailing .git
            if normalized.ends_with(".git") {
                normalized = normalized[..normalized.len() - 4].to_string();
            }

            // Remove trailing slash
            normalized = normalized.trim_end_matches('/').to_string();

            normalized
        };

        let actual_normalized = normalize(actual_url);
        let expected_normalized = normalize(expected_url);

        if actual_normalized != expected_normalized {
            return Err(anyhow::anyhow!(
                "Remote URL mismatch: expected '{}' but found '{}'",
                redact_credentials(expected_url),
                redact_credentials(actual_url)
            ));
        }

        Ok(())
    }

    /// Clone a repository from a remote URL
    ///
    /// This function handles authentication by optionally embedding the token directly in the URL
    /// (format: <https://token@github.com>/...) to bypass gitconfig URL rewrites
    /// (e.g., `url."git@github.com:".insteadOf = "https://github.com/"`).
    ///
    /// # Arguments
    /// * `url` - The remote URL to clone from
    /// * `path` - Local path for the repository
    /// * `token` - Optional GitHub token for authentication
    /// * `embed_credentials` - Whether to embed the token in the URL (default behavior if not specified)
    ///
    /// Note: Consider using `clone_or_open` instead, which handles existing repositories gracefully.
    pub fn clone(url: &str, path: &Path, token: Option<&str>) -> Result<Self> {
        Self::clone_with_options(url, path, token, true)
    }

    /// Clone a repository with explicit control over credential embedding.
    pub fn clone_with_options(
        url: &str,
        path: &Path,
        token: Option<&str>,
        embed_credentials: bool,
    ) -> Result<Self> {
        // Use system git for SSH URLs (libssh2 has compatibility issues with
        // some SSH agents like 1Password, `YubiKey`, Secretive)
        if is_ssh_url(url) {
            clone_via_cli(url, path)?;
            let repo = Repository::open(path)
                .with_context(|| format!("Failed to open cloned repository at {path:?}"))?;
            return Ok(Self { repo });
        }

        // Optionally embed token directly in URL to bypass gitconfig URL rewrites
        // This prevents issues when users have .gitconfig settings like:
        // [url "git@github.com:"]
        //     insteadOf = "https://github.com/"
        let clone_url = if embed_credentials {
            if let Some(token) = token {
                // If token is not already in URL, embed it
                if !url.contains('@') && url.starts_with("https://") {
                    // Insert token after "https://"
                    url.replacen("https://", &format!("https://{token}@"), 1)
                } else {
                    // Token already in URL or not HTTPS, use as-is
                    url.to_string()
                }
            } else {
                url.to_string()
            }
        } else {
            // Don't embed credentials - use URL as-is (or clean it if it has credentials)
            remove_credentials_from_url(url)
        };

        let mut builder = RepoBuilder::new();

        // Set up credentials callback for authentication (used when not embedding in URL)
        if !embed_credentials {
            if let Some(token) = token {
                let token_clone = token.to_string();
                let mut callbacks = RemoteCallbacks::new();
                callbacks.credentials(move |_url, username_from_url, allowed_types| {
                    if !allowed_types.is_user_pass_plaintext() {
                        return Err(git2::Error::from_str(
                            "User/password authentication not allowed",
                        ));
                    }
                    let username = username_from_url.unwrap_or("x-access-token");
                    Cred::userpass_plaintext(username, &token_clone)
                });

                let mut fetch_opts = FetchOptions::new();
                fetch_opts.remote_callbacks(callbacks);
                builder.fetch_options(fetch_opts);
            }
        }

        // Clone with improved error handling
        let repo = builder.clone(&clone_url, path).map_err(|e| {
            // Provide more detailed error message
            let error_msg = e.message();
            anyhow::anyhow!(
                "Failed to clone repository from {url} to {path:?}\n\n\
                Underlying error: {error_msg}\n\n\
                Common causes:\n\
                - Repository URL rewrite in .gitconfig (try: git config --global --unset url.git@github.com:.insteadOf)\n\
                - Invalid or expired GitHub token\n\
                - Network connectivity issues\n\
                - Repository does not exist or is private and token lacks access"
            )
        })?;

        Ok(Self { repo })
    }

    /// Get diff for a specific file as a string
    pub fn get_diff_for_file(&self, path: &str) -> Result<Option<String>> {
        let mut diff_opts = git2::DiffOptions::new();
        diff_opts.pathspec(path);
        diff_opts.context_lines(3); // Standard context

        // 1. Check for unstaged changes (Workdir vs Index)
        let diff_workdir = self
            .repo
            .diff_index_to_workdir(None, Some(&mut diff_opts))
            .context("Failed to get workdir diff")?;

        // 2. Check for staged changes (Index vs HEAD)
        let head_tree = match self.repo.head() {
            Ok(head) => Some(head.peel_to_tree()?),
            Err(_) => None,
        };

        let diff_index = if let Some(tree) = head_tree.as_ref() {
            Some(
                self.repo
                    .diff_tree_to_index(Some(tree), Some(&self.repo.index()?), Some(&mut diff_opts))
                    .context("Failed to get index diff")?,
            )
        } else {
            None
        };

        // Combine diffs or select relevant one
        // If we have both, we probably want to show both or prioritize workdir?
        // Let's format them into a single buffer
        let mut diff_buf = Vec::new();

        // Helper to format a diff into the buffer
        let print_diff = |diff: &git2::Diff, buf: &mut Vec<u8>| -> Result<()> {
            diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
                let origin = line.origin();
                match origin {
                    '+' | '-' | ' ' => {
                        buf.push(origin as u8);
                    }
                    _ => {}
                }
                buf.extend_from_slice(line.content());
                true
            })
            .map_err(|e| anyhow::anyhow!("Diff print error: {e}"))?;
            Ok(())
        };

        if let Some(diff) = diff_index {
            print_diff(&diff, &mut diff_buf)?;
        }
        print_diff(&diff_workdir, &mut diff_buf)?;

        if diff_buf.is_empty() {
            // Might be an untracked file or binary?
            // If it's untracked (New), we might want to just show the file content
            let path_obj = Path::new(path);
            if path_obj.exists() && path_obj.is_file() {
                // Check if it's untracked
                let status = self
                    .repo
                    .status_file(path_obj)
                    .unwrap_or(git2::Status::empty());
                if status.contains(git2::Status::WT_NEW) {
                    return Ok(Some(
                        std::fs::read_to_string(path_obj)
                            .unwrap_or_else(|_| "Binary file or unreadable".to_string()),
                    ));
                }
            }
            return Ok(None);
        }

        Ok(Some(String::from_utf8_lossy(&diff_buf).to_string()))
    }

    /// Check if a remote exists
    #[must_use]
    pub fn has_remote(&self, remote_name: &str) -> bool {
        self.repo.find_remote(remote_name).is_ok()
    }
}

/// Validation result for local repository
#[derive(Debug)]
pub struct LocalRepoValidation {
    pub is_valid: bool,
    #[allow(dead_code)]
    pub has_git: bool,
    #[allow(dead_code)]
    pub has_origin: bool,
    pub remote_url: Option<String>,
    pub error_message: Option<String>,
}

/// Validate a local repository for use with `DotState`
///
/// Checks:
/// 1. Path exists
/// 2. Is a git repository (has .git directory)
/// 3. Has a remote named "origin" configured
#[must_use]
pub fn validate_local_repo(path: &Path) -> LocalRepoValidation {
    // Expand ~ to home directory
    let expanded_path = if path.starts_with("~") {
        if let Some(home) = dirs::home_dir() {
            home.join(path.strip_prefix("~").unwrap_or(path))
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };

    // Check if path exists
    if !expanded_path.exists() {
        return LocalRepoValidation {
            is_valid: false,
            has_git: false,
            has_origin: false,
            remote_url: None,
            error_message: Some(format!("Path does not exist: {}", expanded_path.display())),
        };
    }

    // Check if it's a git repository
    let git_dir = expanded_path.join(".git");
    if !git_dir.exists() {
        return LocalRepoValidation {
            is_valid: false,
            has_git: false,
            has_origin: false,
            remote_url: None,
            error_message: Some(
                "Not a git repository. Run 'git clone <url> <path>' first.".to_string(),
            ),
        };
    }

    // Try to open the repository
    let repo = match Repository::open(&expanded_path) {
        Ok(r) => r,
        Err(e) => {
            return LocalRepoValidation {
                is_valid: false,
                has_git: true,
                has_origin: false,
                remote_url: None,
                error_message: Some(format!("Failed to open repository: {e}")),
            };
        }
    };

    // Check for origin remote
    let remote_url = match repo.find_remote("origin") {
        Ok(remote) => remote.url().map(std::string::ToString::to_string),
        Err(_) => {
            return LocalRepoValidation {
                is_valid: false,
                has_git: true,
                has_origin: false,
                remote_url: None,
                error_message: Some(
                    "No 'origin' remote found. Run 'git remote add origin <url>' first."
                        .to_string(),
                ),
            };
        }
    };

    LocalRepoValidation {
        is_valid: true,
        has_git: true,
        has_origin: true,
        remote_url,
        error_message: None,
    }
}

/// Expand ~ to home directory in a path string
#[must_use]
pub fn expand_path(path_str: &str) -> std::path::PathBuf {
    if let Some(stripped) = path_str.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(stripped);
        }
    } else if let Some(stripped) = path_str.strip_prefix('~') {
        // Handle case of just "~" or "~something" (without slash)
        if let Some(home) = dirs::home_dir() {
            if stripped.is_empty() {
                return home;
            }
            return home.join(stripped);
        }
    }
    std::path::PathBuf::from(path_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_init() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();
        // After initialization, the repo will have a .gitignore file, so it's not empty
        // Check that the repo was successfully initialized instead
        assert!(!git_mgr.repo().is_bare());
        assert!(repo_path.join(".git").exists());
    }

    #[test]
    fn test_generate_commit_message_empty() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();

        // Commit the initial .gitignore file
        git_mgr.commit_all("Initial commit").unwrap();

        // With no changes, should return default message
        let msg = git_mgr.generate_commit_message().unwrap();
        assert_eq!(msg, "Update dotfiles");
    }

    #[test]
    fn test_generate_commit_message_added_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();

        // Commit the initial .gitignore file
        git_mgr.commit_all("Initial commit").unwrap();

        // Add a new file
        std::fs::write(repo_path.join("test.txt"), "test").unwrap();

        let msg = git_mgr.generate_commit_message().unwrap();
        assert!(msg.contains("Add"));
        assert!(msg.contains("test.txt") || msg.contains("file"));
    }

    #[test]
    fn test_generate_commit_message_modified_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();

        // Create and commit a file
        std::fs::write(repo_path.join("test.txt"), "original").unwrap();
        git_mgr.commit_all("Initial commit").unwrap();

        // Modify the file
        std::fs::write(repo_path.join("test.txt"), "modified").unwrap();

        let msg = git_mgr.generate_commit_message().unwrap();
        assert!(msg.contains("Update") || msg.contains("file"));
    }

    #[test]
    fn test_generate_commit_message_multiple_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();

        // Commit the initial .gitignore file
        git_mgr.commit_all("Initial commit").unwrap();

        // Add multiple files
        std::fs::write(repo_path.join("file1.txt"), "test1").unwrap();
        std::fs::write(repo_path.join("file2.txt"), "test2").unwrap();
        std::fs::write(repo_path.join("file3.txt"), "test3").unwrap();

        let msg = git_mgr.generate_commit_message().unwrap();
        assert!(msg.contains("Add"));
        assert!(msg.contains('3') || msg.contains("file"));
    }

    #[test]
    fn test_generate_commit_message_manifest_only() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();

        // Commit the initial .gitignore file
        git_mgr.commit_all("Initial commit").unwrap();

        // Create and commit the manifest file (as it would be in real usage)
        std::fs::write(repo_path.join(".dotstate-profiles.toml"), "[profiles]\n").unwrap();
        git_mgr.commit_all("Add manifest").unwrap();

        // Now modify it (this simulates adding a dependency or creating a new profile)
        std::fs::write(
            repo_path.join(".dotstate-profiles.toml"),
            "[profiles]\nname = \"test\"\n",
        )
        .unwrap();

        let msg = git_mgr.generate_commit_message().unwrap();
        // Should return "Update profile configuration" since only manifest was modified
        assert_eq!(msg, "Update profile configuration");
    }

    #[test]
    fn test_generate_commit_message_manifest_with_other_files() {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();
        let git_mgr = GitManager::open_or_init(repo_path).unwrap();

        // Commit the initial .gitignore file
        git_mgr.commit_all("Initial commit").unwrap();

        // Modify both manifest and another file
        std::fs::write(repo_path.join(".dotstate-profiles.toml"), "[profiles]\n").unwrap();
        std::fs::write(repo_path.join("test.txt"), "test").unwrap();

        let msg = git_mgr.generate_commit_message().unwrap();
        // Should ignore manifest and only mention test.txt
        assert!(msg.contains("Add") || msg.contains("Update"));
        assert!(!msg.contains("profile configuration"));
    }

    #[test]
    fn test_validate_local_repo_nonexistent() {
        let result = validate_local_repo(Path::new("/nonexistent/path"));
        assert!(!result.is_valid);
        assert!(!result.has_git);
        assert!(!result.has_origin);
        assert!(result.error_message.is_some());
    }

    #[test]
    fn test_validate_local_repo_not_git() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_local_repo(temp_dir.path());
        assert!(!result.is_valid);
        assert!(!result.has_git);
        assert!(!result.has_origin);
        assert!(result
            .error_message
            .unwrap()
            .contains("Not a git repository"));
    }

    #[test]
    fn test_validate_local_repo_no_origin() {
        let temp_dir = TempDir::new().unwrap();
        // Initialize a repo without origin
        let _git_mgr = GitManager::open_or_init(temp_dir.path()).unwrap();

        let result = validate_local_repo(temp_dir.path());
        assert!(!result.is_valid);
        assert!(result.has_git);
        assert!(!result.has_origin);
        assert!(result.error_message.unwrap().contains("No 'origin' remote"));
    }

    #[test]
    fn test_validate_local_repo_valid() {
        let temp_dir = TempDir::new().unwrap();
        // Initialize a repo with origin
        let mut git_mgr = GitManager::open_or_init(temp_dir.path()).unwrap();
        git_mgr
            .add_remote("origin", "https://github.com/test/test.git")
            .unwrap();

        let result = validate_local_repo(temp_dir.path());
        assert!(result.is_valid);
        assert!(result.has_git);
        assert!(result.has_origin);
        assert!(result.error_message.is_none());
        assert_eq!(
            result.remote_url,
            Some("https://github.com/test/test.git".to_string())
        );
    }

    #[test]
    fn test_expand_path() {
        let home = dirs::home_dir().unwrap();

        // Test ~ expansion
        let expanded = expand_path("~/test");
        assert_eq!(expanded, home.join("test"));

        // Test without ~
        let no_expand = expand_path("/absolute/path");
        assert_eq!(no_expand, std::path::PathBuf::from("/absolute/path"));
    }
}
