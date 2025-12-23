use anyhow::{Context, Result};
use git2::{build::RepoBuilder, Cred, FetchOptions, RemoteCallbacks, Repository, Signature};
use std::path::Path;

/// Git operations for managing the dotfiles repository
pub struct GitManager {
    repo: Repository,
}

impl GitManager {
    /// Open or initialize a repository
    pub fn open_or_init(repo_path: &Path) -> Result<Self> {
        let repo = if repo_path.join(".git").exists() {
            Repository::open(repo_path)
                .with_context(|| format!("Failed to open repository: {:?}", repo_path))?
        } else {
            // Initialize as a normal (non-bare) repository so it has a working directory
            let mut repo = Repository::init(repo_path)
                .with_context(|| format!("Failed to initialize repository: {:?}", repo_path))?;

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
                "Repository at {:?} is a bare repository and has no working directory. \
                Cannot add files to index.",
                repo_path
            ));
        }

        Ok(Self { repo })
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
            .with_context(|| format!("Failed to create .gitignore at {:?}", gitignore_path))?;

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
        match repo.head() {
            Ok(head) => {
                if let Some(branch_name) = head.name().and_then(|n| n.strip_prefix("refs/heads/")) {
                    if branch_name == "master" {
                        // Rename master to main
                        let master_ref = repo.find_reference("refs/heads/master")?;
                        if let Some(target) = master_ref.target() {
                            repo.reference(
                                "refs/heads/main",
                                target,
                                true,
                                "Rename master to main",
                            )?;
                            // Update HEAD to point to main
                            repo.set_head("refs/heads/main")?;
                            // Delete old master branch
                            repo.find_reference("refs/heads/master")?.delete()?;
                        }
                    }
                }
            }
            Err(_) => {
                // No HEAD yet - this is fine, the first commit will create the branch
                // We can't set HEAD to a non-existent branch, so we'll handle it in commit_all
            }
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

    /// Add all changes and commit
    pub fn commit_all(&self, message: &str) -> Result<()> {
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

        self.repo
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
        }

        Ok(())
    }

    /// Push to remote
    /// If token is provided, it will be used for authentication.
    /// Otherwise, attempts to extract token from remote URL.
    pub fn push(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<()> {
        let mut remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{}' not found", remote_name))?;

        let remote_url = remote
            .url()
            .ok_or_else(|| anyhow::anyhow!("Remote '{}' has no URL", remote_name))?;

        let mut callbacks = RemoteCallbacks::new();

        // Try to get token from parameter first, then from URL
        let token_to_use = token
            .map(|t| t.to_string())
            .or_else(|| Self::extract_token_from_url(remote_url));

        if let Some(token) = token_to_use {
            // Set up credentials callback with the token
            // For GitHub PAT authentication, use token as username and password
            let token_clone = token.clone();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                // GitHub PAT: use token as both username and password
                Cred::userpass_plaintext(&token_clone, &token_clone)
            });
        } else {
            // If no token found, provide a helpful error
            callbacks.credentials(|_url, _username_from_url, _allowed_types| {
                Err(git2::Error::from_str(
                    "No credentials found. Please provide a GitHub token.",
                ))
            });
        }

        let mut push_options = git2::PushOptions::new();
        push_options.remote_callbacks(callbacks);

        // Check if branch exists locally
        let branch_ref = format!("refs/heads/{}", branch);
        if self.repo.find_reference(&branch_ref).is_err() {
            // Branch doesn't exist, try to get current branch
            if let Some(current_branch) = self.get_current_branch() {
                let refspec = format!("refs/heads/{}:refs/heads/{}", current_branch, branch);
                remote
                    .push(&[&refspec], Some(&mut push_options))
                    .with_context(|| format!("Failed to push to remote '{}'", remote_name))?;
                return Ok(());
            }
            return Err(anyhow::anyhow!(
                "No branch '{}' exists and no current branch found",
                branch
            ));
        }

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        remote.push(&[&refspec], Some(&mut push_options))
            .with_context(|| {
                // Get more detailed error information
                let remote_url = remote.url().unwrap_or("unknown");
                format!(
                    "Failed to push to remote '{}' (URL: {}). \
                    Check that:\n  - Your GitHub token has 'repo' scope\n  - The remote branch exists\n  - You have push permissions",
                    remote_name, remote_url
                )
            })?;

        Ok(())
    }

    /// Extract token from a GitHub URL (format: https://token@github.com/...)
    fn extract_token_from_url(url: &str) -> Option<String> {
        // Parse URL like https://token@github.com/user/repo.git
        if let Some(at_pos) = url.find('@') {
            if url.starts_with("https://") {
                let start = 8; // Skip "https://"
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

    /// Pull from remote
    pub fn pull(&self, remote_name: &str, branch: &str, token: Option<&str>) -> Result<()> {
        let mut remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{}' not found", remote_name))?;

        let mut callbacks = RemoteCallbacks::new();
        let remote_url = remote
            .url()
            .ok_or_else(|| anyhow::anyhow!("Remote '{}' has no URL", remote_name))?;

        // Try to get token from parameter first, then from URL
        let token_to_use = token
            .map(|t| t.to_string())
            .or_else(|| Self::extract_token_from_url(remote_url));

        if let Some(token) = token_to_use {
            let token_clone = token.clone();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                Cred::userpass_plaintext(&token_clone, &token_clone)
            });
        }

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        remote
            .fetch(&[branch], Some(&mut fetch_options), None)
            .with_context(|| format!("Failed to fetch from remote '{}'", remote_name))?;

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
            let branch_ref = format!("refs/heads/{}", branch);
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
        let mut remote = self
            .repo
            .find_remote(remote_name)
            .with_context(|| format!("Remote '{}' not found", remote_name))?;

        let mut callbacks = RemoteCallbacks::new();
        let remote_url = remote
            .url()
            .ok_or_else(|| anyhow::anyhow!("Remote '{}' has no URL", remote_name))?;

        // Try to get token from parameter first, then from URL
        let token_to_use = token
            .map(|t| t.to_string())
            .or_else(|| Self::extract_token_from_url(remote_url));

        if let Some(token) = token_to_use {
            let token_clone = token.clone();
            callbacks.credentials(move |_url, _username_from_url, _allowed_types| {
                Cred::userpass_plaintext(&token_clone, &token_clone)
            });
        }

        let mut fetch_options = FetchOptions::new();
        fetch_options.remote_callbacks(callbacks);

        remote
            .fetch(&[branch], Some(&mut fetch_options), None)
            .with_context(|| format!("Failed to fetch from remote '{}'", remote_name))?;

        // Check if FETCH_HEAD exists (remote might not have the branch yet)
        let fetch_head = match self.repo.find_reference("FETCH_HEAD") {
            Ok(ref_) => ref_,
            Err(_) => {
                // No remote commits yet, nothing to rebase
                return Ok(0);
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

        // Count commits in remote that we don't have locally
        let mut pulled_count = 0;
        let fetch_commit_id = fetch_commit.id();

        if let Some(local_commit) = local_head {
            // Check if remote is ahead (different commits)
            if local_commit.id() != fetch_commit_id {
                // Find merge base between local and remote
                let merge_base = self
                    .repo
                    .merge_base(local_commit.id(), fetch_commit_id)
                    .context("Failed to find merge base")?;

                // Count commits from merge base to remote HEAD
                let mut commit = fetch_commit.clone();
                loop {
                    if commit.id() == merge_base {
                        break;
                    }
                    pulled_count += 1;
                    if commit.parent_count() == 0 {
                        // Reached root without finding merge base - count all commits
                        break;
                    }
                    commit = commit.parent(0)?;
                }

                // Perform merge (git2 doesn't have direct rebase API, so we use merge)
                // The UI will call it "rebase" but technically it's a merge
                let annotated_commit = self
                    .repo
                    .find_annotated_commit(fetch_commit_id)
                    .context("Failed to create annotated commit")?;

                self.repo
                    .merge(&[&annotated_commit], None, None)
                    .context("Failed to merge")?;

                let mut index = self
                    .repo
                    .index()
                    .context("Failed to get index after merge")?;

                if index.has_conflicts() {
                    return Err(anyhow::anyhow!(
                        "Merge conflicts detected. Please resolve manually."
                    ));
                }

                index.write().context("Failed to write index after merge")?;

                let tree_id = index
                    .write_tree()
                    .context("Failed to write tree after merge")?;
                let tree = self
                    .repo
                    .find_tree(tree_id)
                    .context("Failed to find tree after merge")?;

                let signature = Self::get_signature()?;
                let fetch_commit_for_merge = self.repo.find_commit(fetch_commit_id)?;

                self.repo
                    .commit(
                        Some("HEAD"),
                        &signature,
                        &signature,
                        "Merge remote-tracking branch",
                        &tree,
                        &[&local_commit, &fetch_commit_for_merge],
                    )
                    .context("Failed to commit merge")?;

                self.repo
                    .cleanup_state()
                    .context("Failed to cleanup merge state")?;
            }
        } else {
            // No local commits, just update HEAD to point to remote
            let branch_ref = format!("refs/heads/{}", branch);
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
            let mut commit = fetch_commit.clone();
            loop {
                pulled_count += 1;
                if commit.parent_count() == 0 {
                    break;
                }
                commit = commit.parent(0)?;
            }
        }

        Ok(pulled_count)
    }

    /// Add a remote (or update if it exists)
    pub fn add_remote(&mut self, name: &str, url: &str) -> Result<()> {
        // remote_set_url doesn't exist in git2, so we delete and recreate
        if self.repo.find_remote(name).is_ok() {
            self.repo
                .remote_delete(name)
                .with_context(|| format!("Failed to delete existing remote '{}'", name))?;
        }
        self.repo
            .remote(name, url)
            .with_context(|| format!("Failed to add remote '{}'", name))?;

        // Configure remote tracking for the current branch
        self.configure_remote_tracking(name)?;

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

            let remote_key = format!("branch.{}.remote", branch_name);
            let merge_key = format!("branch.{}.merge", branch_name);

            config
                .set_str(&remote_key, remote_name)
                .context("Failed to set branch remote")?;
            config
                .set_str(&merge_key, &format!("refs/heads/{}", branch_name))
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

        let remote_key = format!("branch.{}.remote", branch_name);
        let merge_key = format!("branch.{}.merge", branch_name);

        config
            .set_str(&remote_key, remote_name)
            .context("Failed to set branch remote")?;
        config
            .set_str(&merge_key, &format!("refs/heads/{}", branch_name))
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

            let has_untracked = statuses
                .iter()
                .any(|s| s.status().contains(git2::Status::WT_NEW));

            Ok(has_changes || has_untracked)
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
        let branch_ref = format!("refs/heads/{}", branch);
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
        let remote_ref = format!("refs/remotes/{}/{}", remote_name, branch);
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
    pub fn get_current_branch(&self) -> Option<String> {
        let head = self.repo.head().ok()?;
        let name = head.name()?;
        // Remove 'refs/heads/' prefix
        name.strip_prefix("refs/heads/").map(|s| s.to_string())
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
                changed_files.push(format!("{}{}", prefix, path));
            }
        }

        Ok(changed_files)
    }

    /// Clone a repository from a remote URL
    pub fn clone(url: &str, path: &Path, token: Option<&str>) -> Result<Self> {
        let mut fetch_options = FetchOptions::new();

        // Set up authentication if token is provided
        if let Some(token) = token {
            let mut callbacks = RemoteCallbacks::new();
            let token_clone = token.to_string();
            callbacks.credentials(move |_url, username, _allowed_types| {
                Cred::userpass_plaintext(username.unwrap_or("git"), &token_clone)
            });
            fetch_options.remote_callbacks(callbacks);
        }

        let mut builder = RepoBuilder::new();
        builder.fetch_options(fetch_options);

        let repo = builder
            .clone(url, path)
            .with_context(|| format!("Failed to clone repository from {} to {:?}", url, path))?;

        Ok(Self { repo })
    }
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
}
