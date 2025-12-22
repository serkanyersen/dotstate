use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Write;
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

    /// Get the current user
    pub async fn get_user(&self) -> Result<GitHubUser> {
        let url = "https://api.github.com/user";
        let auth_header = format!("token {}", self.token);

        // Log request details (mask token for security)
        let token_preview = if self.token.len() > 8 {
            format!("{}...{}", &self.token[..4], &self.token[self.token.len()-4..])
        } else {
            "***".to_string()
        };

        info!("=== GitHub API Request ===");
        info!("URL: {}", url);
        info!("Method: GET");
        info!("Token preview: {}", token_preview);
        info!("Token length: {} characters", self.token.len());
        info!("Token starts with: {}", if self.token.starts_with("ghp_") { "ghp_ (correct)" } else { &self.token[..self.token.len().min(4)] });
        info!("Authorization header: token {}", token_preview);
        info!("User-Agent: dotstate");
        info!("Accept: application/vnd.github.v3+json");
        info!("Full auth header value (first 20 chars): {}", &auth_header[..auth_header.len().min(20)]);

        let request = self
            .http_client
            .get(url)
            .header("Authorization", &auth_header)
            .header("User-Agent", "dotstate")
            .header("Accept", "application/vnd.github.v3+json");

        // Log the actual request being built
        debug!("Request built, sending...");

        let response = request
            .send()
            .await
            .context("Failed to fetch user")?;

        let status = response.status();
        info!("=== GitHub API Response ===");
        info!("Status: {} {}", status.as_u16(), status);

        // Log response headers
        let headers = response.headers();
        info!("Response headers:");
        for (name, value) in headers.iter() {
            if let Ok(value_str) = value.to_str() {
                info!("  {}: {}", name, value_str);
            }
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            error!("=== GitHub API Error ===");
            error!("Status: {} {}", status.as_u16(), status);
            error!("Error response body: {}", error_text);
            error!("Request was: GET {}", url);
            error!("Auth header format used: token <token>");
            error!("Token length: {} chars", self.token.len());

            if status == reqwest::StatusCode::UNAUTHORIZED {
                error!("Unauthorized - token may be invalid, expired, or have wrong format");
                anyhow::bail!(
                    "Invalid token or insufficient permissions.\n\n\
                    Common issues:\n\
                    • Token may be expired - check https://github.com/settings/tokens\n\
                    • Token may have been revoked\n\
                    • Make sure you copied the entire token (starts with 'ghp_')\n\
                    • Token should be 40+ characters long\n\
                    • For CLASSIC tokens: 'repo' scope should be checked\n\
                    • Try generating a new token if this one doesn't work\n\n\
                    Check console/logs for detailed request information."
                );
            }

            anyhow::bail!("GitHub API error ({}): {}", status, error_text);
        }

        let user: GitHubUser = response
            .json()
            .await
            .context("Failed to parse user response. The token may be invalid.")?;

        Ok(user)
    }

    /// Check if a repository exists
    pub async fn repo_exists(&self, owner: &str, repo: &str) -> Result<bool> {
        let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
        info!("Checking if repository exists: {}", url);

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
        info!("Repository check status: {}", status);

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

    /// Create a new repository
    pub async fn create_repo(&self, name: &str, description: &str, private: bool) -> Result<GitHubRepo> {
        let request_body = CreateRepoRequest {
            name: name.to_string(),
            description: description.to_string(),
            private,
            auto_init: false, // Don't auto-initialize - we'll create our own initial commit
        };

        let url = "https://api.github.com/user/repos";
        let auth_header = format!("token {}", self.token);
        let token_preview = if self.token.len() > 8 {
            format!("{}...{}", &self.token[..4], &self.token[self.token.len()-4..])
        } else {
            "***".to_string()
        };

        info!("=== GitHub API Request (Create Repo) ===");
        info!("URL: {}", url);
        info!("Method: POST");
        info!("Authorization header: token {}", token_preview);
        info!("User-Agent: dotstate");
        info!("Accept: application/vnd.github.v3+json");
        info!("Request body: name={}, description={}, private={}, auto_init=false", name, description, private);

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
        info!("=== GitHub API Response (Create Repo) ===");
        info!("Status: {} {}", status.as_u16(), status);

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!("Create repository error response: {}", error_text);

            if status == reqwest::StatusCode::FORBIDDEN {
                error!("Forbidden - token lacks permission to create repositories");
                anyhow::bail!(
                    "Insufficient permissions to create repository.\n\n\
                    Your token doesn't have permission to create repositories.\n\
                    For CLASSIC tokens: Make sure you selected 'repo' scope (full control).\n\
                    For FINE-GRAINED tokens: They cannot create repositories.\n\
                    Please use a classic token with 'repo' scope for first-time setup.\n\n\
                    Check console/logs for detailed request information."
                );
            }

            anyhow::bail!("Failed to create repository ({}): {}", status, error_text);
        }

        let repo: GitHubRepo = response
            .json()
            .await
            .context("Failed to parse repository response")?;

        Ok(repo)
    }

}

/// Simplified OAuth flow using Personal Access Token
/// This is a fallback method that's easier for users
#[allow(dead_code)]
pub async fn authenticate_with_pat() -> Result<String> {
    // For now, we'll use a simple prompt
    // In the TUI, this will be a proper input field
    println!("Please enter your GitHub Personal Access Token:");
    println!("You can create one at: https://github.com/settings/tokens");
    println!("Required scopes: repo (full control of private repositories)");
    print!("Token: ");
    std::io::stdout().flush()?;

    let mut token = String::new();
    std::io::stdin().read_line(&mut token)?;
    token = token.trim().to_string();

    if token.is_empty() {
        anyhow::bail!("Token cannot be empty");
    }

    // Verify token works
    let client = Client::new();
    let response = client
        .get("https://api.github.com/user")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "dotstate")
        .send()
        .await
        .context("Failed to verify token")?;

    if !response.status().is_success() {
        anyhow::bail!("Invalid token. Please check your Personal Access Token.");
    }

    Ok(token)
}

