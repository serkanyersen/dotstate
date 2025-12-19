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
            Repository::init(repo_path)
                .with_context(|| format!("Failed to initialize repository: {:?}", repo_path))?
        };

        Ok(Self { repo })
    }

    /// Add all changes and commit
    pub fn commit_all(&self, message: &str) -> Result<()> {
        let mut index = self.repo.index()
            .context("Failed to get repository index")?;

        // Use empty pathspec to add all files, or use "*" pattern
        // Empty slice means add all files
        index.add_all(&[] as &[&str], git2::IndexAddOption::DEFAULT, None)
            .context("Failed to add files to index")?;

        index.write()
            .context("Failed to write index")?;

        let tree_id = index.write_tree()
            .context("Failed to write tree")?;
        let tree = self.repo.find_tree(tree_id)
            .context("Failed to find tree")?;

        let signature = Self::get_signature()?;
        let head = self.repo.head();

        let parent_commit = if let Ok(head) = head {
            Some(head.peel_to_commit()
                .context("Failed to peel HEAD to commit")?)
        } else {
            None
        };

        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .context("Failed to create commit")?;

        Ok(())
    }

    /// Push to remote
    pub fn push(&self, remote_name: &str, branch: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)
            .with_context(|| format!("Remote '{}' not found", remote_name))?;

        // Check if branch exists locally
        let branch_ref = format!("refs/heads/{}", branch);
        if self.repo.find_reference(&branch_ref).is_err() {
            // Branch doesn't exist, try to get current branch
            if let Some(current_branch) = self.get_current_branch() {
                let refspec = format!("refs/heads/{}:refs/heads/{}", current_branch, branch);
                remote.push(&[&refspec], None)
                    .with_context(|| format!("Failed to push to remote '{}'", remote_name))?;
                return Ok(());
            }
            return Err(anyhow::anyhow!("No branch '{}' exists and no current branch found", branch));
        }

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);
        remote.push(&[&refspec], None)
            .with_context(|| format!("Failed to push to remote '{}'", remote_name))?;

        Ok(())
    }

    /// Pull from remote
    pub fn pull(&self, remote_name: &str, branch: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name)
            .with_context(|| format!("Remote '{}' not found", remote_name))?;

        remote.fetch(&[branch], None, None)
            .with_context(|| format!("Failed to fetch from remote '{}'", remote_name))?;

        let fetch_head = self.repo.find_reference("FETCH_HEAD")
            .context("Failed to find FETCH_HEAD")?;
        let fetch_commit = fetch_head.peel_to_commit()
            .context("Failed to peel FETCH_HEAD to commit")?;

        // Convert commit to annotated commit for merge
        let annotated_commit = self.repo.find_annotated_commit(fetch_commit.id())
            .context("Failed to create annotated commit")?;

        self.repo.merge(&[&annotated_commit], None, None)
            .context("Failed to merge")?;

        Ok(())
    }

    /// Add a remote (or update if it exists)
    pub fn add_remote(&mut self, name: &str, url: &str) -> Result<()> {
        // remote_set_url doesn't exist in git2, so we delete and recreate
        if self.repo.find_remote(name).is_ok() {
            self.repo.remote_delete(name)
                .with_context(|| format!("Failed to delete existing remote '{}'", name))?;
        }
        self.repo.remote(name, url)
            .with_context(|| format!("Failed to add remote '{}'", name))?;
        Ok(())
    }

    /// Get signature for commits
    fn get_signature() -> Result<Signature<'static>> {
        // Try to get from git config, fallback to defaults
        let config = git2::Config::open_default().ok();

        let name = config
            .as_ref()
            .and_then(|c| c.get_string("user.name").ok())
            .unwrap_or_else(|| "dotzz".to_string());

        let email = config
            .as_ref()
            .and_then(|c| c.get_string("user.email").ok())
            .unwrap_or_else(|| "dotzz@localhost".to_string());

        Ok(Signature::now(&name, &email)?)
    }

    /// Get the repository reference
    #[allow(dead_code)]
    pub fn repo(&self) -> &Repository {
        &self.repo
    }

    /// Check if there are uncommitted changes
    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let mut index = self.repo.index()
            .context("Failed to get repository index")?;

        // Refresh the index to get current state
        index.read(true)
            .context("Failed to read index")?;

        // Check if index differs from HEAD
        let head = match self.repo.head() {
            Ok(head) => Some(head.peel_to_tree()
                .context("Failed to peel HEAD to tree")?),
            Err(_) => None,
        };

        if let Some(head_tree) = head {
            let diff = self.repo.diff_tree_to_index(
                Some(&head_tree),
                Some(&index),
                None,
            )
            .context("Failed to create diff")?;

            // Check if there are any differences
            let has_changes = diff.deltas().next().is_some();

            // Also check for untracked files
            let mut status_opts = git2::StatusOptions::new();
            status_opts.include_untracked(true);
            status_opts.include_ignored(false);

            let statuses = self.repo.statuses(Some(&mut status_opts))
                .context("Failed to get status")?;

            let has_untracked = statuses.iter().any(|s| {
                s.status().contains(git2::Status::WT_NEW)
            });

            Ok(has_changes || has_untracked)
        } else {
            // No HEAD, check if index has any entries
            Ok(index.len() > 0)
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

        let local_oid = local_branch.target()
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

        let remote_oid = remote_branch.target()
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

        let repo = builder.clone(url, path)
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
        let git_mgr = GitManager::open_or_init(temp_dir.path()).unwrap();
        assert!(git_mgr.repo().is_empty().unwrap_or(false));
    }
}


