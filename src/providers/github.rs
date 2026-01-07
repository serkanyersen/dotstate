use crate::providers::{GitProvider, RepoInfo};
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info};

/// GitHub API client for repository operations
pub struct GitHubClient {
    http_client: Client,
    token: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubUser {
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct GitHubRepo {
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub full_name: String,
    #[allow(dead_code)]
    pub default_branch: String,
    #[allow(dead_code)]
    pub clone_url: String,
}

#[derive(Debug, Serialize)]
struct CreateRepoRequest {
    name: String,
    description: String,
    private: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    auto_init: bool, // Set to false - we'll create our own initial commit
}

impl GitHubClient {
    /// Create a new GitHub client with a token
    pub fn new(token: String) -> Self {
        Self {
            http_client: Client::new(),
            token,
        }
    }
}

#[async_trait]
impl GitProvider for GitHubClient {
    fn id(&self) -> &str {
        "github"
    }

    fn display_name(&self) -> &str {
        "GitHub"
    }

    async fn authenticate(&self) -> Result<String> {
        let url = "https://api.github.com/user";
        let auth_header = format!("token {}", self.token);

        info!("Authenticating with GitHub...");

        let request = self
            .http_client
            .get(url)
            .header("Authorization", &auth_header)
            .header("User-Agent", "dotstate")
            .header("Accept", "application/vnd.github.v3+json");

        let response = request.send().await.context("Failed to fetch user")?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());

            if status == reqwest::StatusCode::UNAUTHORIZED {
                anyhow::bail!(
                    "Invalid token or insufficient permissions.\n\n\
                    Common issues:\n\
                    • Token may be expired\n\
                    • Token may have been revoked\n\
                    • Make sure you copied the entire token (starts with 'ghp_')\n"
                );
            }
            anyhow::bail!("GitHub API error ({}): {}", status, error_text);
        }

        let user: GitHubUser = response
            .json()
            .await
            .context("Failed to parse user response")?;

        Ok(user.login)
    }

    async fn repo_exists(&self, owner: &str, repo: &str) -> Result<bool> {
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("token {}", self.token))
            .header("User-Agent", "dotstate")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .context("Failed to check repository")?;

        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Failed to check repository ({}): {}", status, error_text);
        }

        Ok(true)
    }

    async fn create_repo(&self, name: &str, description: &str, private: bool) -> Result<RepoInfo> {
        let request_body = CreateRepoRequest {
            name: name.to_string(),
            description: description.to_string(),
            private,
            auto_init: false,
        };

        let url = "https://api.github.com/user/repos";
        let auth_header = format!("token {}", self.token);

        let response = self
            .http_client
            .post(url)
            .header("Authorization", &auth_header)
            .header("User-Agent", "dotstate")
            .header("Accept", "application/vnd.github.v3+json")
            .json(&request_body)
            .send()
            .await
            .context("Failed to create repository")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();

            if status == reqwest::StatusCode::FORBIDDEN {
                anyhow::bail!(
                    "Insufficient permissions to create repository.\n\
                    Your token doesn't have permission to create repositories."
                );
            }
            anyhow::bail!("Failed to create repository ({}): {}", status, error_text);
        }

        let repo: GitHubRepo = response
            .json()
            .await
            .context("Failed to parse repository response")?;

        Ok(RepoInfo {
            name: repo.name,
            clone_url: repo.clone_url,
            default_branch: repo.default_branch,
        })
    }

    fn get_remote_url(&self, owner: &str, repo: &str) -> String {
        format!("https://{}@github.com/{}/{}.git", self.token, owner, repo)
    }

    fn get_setup_instructions(&self) -> String {
        "Please enter your GitHub Personal Access Token.\n\
        You can create one at: https://github.com/settings/tokens\n\
        Required scopes: 'repo' (full control of private repositories)".to_string()
    }
}
