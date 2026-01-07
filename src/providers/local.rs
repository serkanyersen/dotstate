use crate::providers::{GitProvider, RepoInfo};
use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

pub struct LocalProvider {
    pub remote_path: Option<PathBuf>,
}

#[async_trait]
impl GitProvider for LocalProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn display_name(&self) -> &str {
        "Local"
    }

    async fn authenticate(&self) -> Result<String> {
        // No authentication needed for local
        Ok("local-user".to_string())
    }

    async fn repo_exists(&self, _owner: &str, _name: &str) -> Result<bool> {
        // For local, "repo exists" means the directory exists and has .git
        // But here we are checking the REMOTE.
        if let Some(path) = &self.remote_path {
            Ok(path.exists())
        } else {
            // No remote, so "remote repo" effectively doesn't exist or doesn't matter
            Ok(false)
        }
    }

    async fn create_repo(&self, name: &str, _description: &str, _private: bool) -> Result<RepoInfo> {
        // If remote path is set, initialize a bare repo there
        if let Some(path) = &self.remote_path {
            if !path.exists() {
                std::fs::create_dir_all(path)?;
                std::process::Command::new("git")
                    .current_dir(path)
                    .args(&["init", "--bare"])
                    .output()?;
            }
            Ok(RepoInfo {
                name: name.to_string(),
                clone_url: path.to_string_lossy().to_string(),
                default_branch: "main".to_string(),
            })
        } else {
            // No remote
             Ok(RepoInfo {
                name: name.to_string(),
                clone_url: String::new(), // Special empty string for no remote
                default_branch: "main".to_string(),
            })
        }
    }

    fn get_remote_url(&self, _owner: &str, _repo: &str) -> String {
        if let Some(path) = &self.remote_path {
            path.to_string_lossy().to_string()
        } else {
            String::new()
        }
    }

    fn get_setup_instructions(&self) -> String {
        "Local setup: Enter path to store your dotfiles locally.".to_string()
    }
}
