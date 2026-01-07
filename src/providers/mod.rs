pub mod github;
pub mod local;

use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct RepoInfo {
    pub name: String,
    pub clone_url: String, // HTTP clone URL
    pub default_branch: String,
}

#[async_trait]
pub trait GitProvider: Send + Sync {
    /// Get the provider type (e.g., "github", "gitlab", "local")
    fn id(&self) -> &str;

    /// Get the display name (e.g., "GitHub", "GitLab.com", "My Forgejo")
    fn display_name(&self) -> &str;

    /// Authenticate the user (verify token) -> Returns username
    async fn authenticate(&self) -> Result<String>;

    /// Check if a repository exists
    async fn repo_exists(&self, owner: &str, name: &str) -> Result<bool>;

    /// Create a new repository
    async fn create_repo(&self, name: &str, description: &str, private: bool) -> Result<RepoInfo>;

    /// Get the remote URL for git operations (e.g. https://github.com/user/repo.git)
    fn get_remote_url(&self, owner: &str, repo: &str) -> String;

    /// Get the setup instructions for this provider (to display in TUI)
    fn get_setup_instructions(&self) -> String;
}
