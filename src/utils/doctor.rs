use anyhow::Result;
use crossterm::style::{Attribute, Color, Stylize};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Write};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::utils::{ProfileManifest, SymlinkManager};

// ============================================================================
// Types and Structs
// ============================================================================

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum ValidationStatus {
    Pass,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub category: String,
    pub check_name: String,
    pub message: String,
    pub status: ValidationStatus,
    pub fixable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix_action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Vec<String>>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorSummary {
    pub total_checks: usize,
    pub passed: usize,
    pub warnings: usize,
    pub errors: usize,
    pub fixable: usize,
    pub fixed: usize,
    pub total_duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub timestamp: String,
    pub results: Vec<ValidationResult>,
    pub summary: DoctorSummary,
}

pub struct DoctorOptions {
    pub fix_mode: bool,
    pub verbose: bool,
    pub json_output: bool,
}

pub struct Doctor {
    config: Config,
    options: DoctorOptions,
    results: Vec<ValidationResult>,
    start_time: Instant,
}

// ============================================================================
// Check Categories
// ============================================================================

struct CheckCategory {
    name: &'static str,
    icon: &'static str,
    description: &'static str,
}

const CATEGORIES: &[CheckCategory] = &[
    CheckCategory {
        name: "Environment",
        icon: "🖥️",
        description: "System environment and dependencies",
    },
    CheckCategory {
        name: "Configuration",
        icon: "⚙️",
        description: "DotState configuration files",
    },
    CheckCategory {
        name: "Repository",
        icon: "📦",
        description: "Git repository status",
    },
    CheckCategory {
        name: "Profiles",
        icon: "👤",
        description: "Profile and manifest integrity",
    },
    CheckCategory {
        name: "Symlinks",
        icon: "🔗",
        description: "Symlink tracking and validity",
    },
    CheckCategory {
        name: "Backups",
        icon: "💾",
        description: "Backup file integrity",
    },
    CheckCategory {
        name: "Filesystem",
        icon: "📁",
        description: "Filesystem permissions and space",
    },
];

// ============================================================================
// Output Helpers
// ============================================================================

fn print_header(title: &str) {
    let width = 60;
    let padding = (width - title.len() - 2) / 2;
    println!();
    println!(
        "{}",
        format!("╭{}╮", "─".repeat(width - 2)).with(Color::DarkCyan)
    );
    println!(
        "{}",
        format!(
            "│{}{} {}{}│",
            " ".repeat(padding),
            title,
            " ".repeat(width - padding - title.len() - 3),
            ""
        )
        .with(Color::DarkCyan)
        .attribute(Attribute::Bold)
    );
    println!(
        "{}",
        format!("╰{}╯", "─".repeat(width - 2)).with(Color::DarkCyan)
    );
    println!();
}

fn print_category_header(category: &CheckCategory, index: usize, total: usize) {
    println!(
        "\n{} {} {} {}",
        format!("[{}/{}]", index + 1, total).with(Color::DarkGrey),
        category.icon,
        category.name.with(Color::Cyan).attribute(Attribute::Bold),
        format!("- {}", category.description).with(Color::DarkGrey)
    );
    println!("{}", "─".repeat(50).with(Color::DarkGrey));
}

fn format_duration(duration: Duration) -> String {
    let ms = duration.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

fn status_icon(status: &ValidationStatus) -> String {
    match status {
        ValidationStatus::Pass => "✓".with(Color::Green).to_string(),
        ValidationStatus::Warning => "⚠".with(Color::Yellow).to_string(),
        ValidationStatus::Error => "✗".with(Color::Red).to_string(),
    }
}

fn print_check_result(result: &ValidationResult, verbose: bool) {
    let icon = status_icon(&result.status);
    let duration = format!(
        "({})",
        format_duration(Duration::from_millis(result.duration_ms))
    );

    let message_color = match result.status {
        ValidationStatus::Pass => Color::White,
        ValidationStatus::Warning => Color::Yellow,
        ValidationStatus::Error => Color::Red,
    };

    println!(
        "  {} {} {}",
        icon,
        result.message.clone().with(message_color),
        duration.with(Color::DarkGrey)
    );

    if verbose {
        if let Some(details) = &result.details {
            for detail in details {
                println!(
                    "      {} {}",
                    "│".with(Color::DarkGrey),
                    detail.clone().with(Color::DarkGrey)
                );
            }
        }
    }

    if result.fixable && result.status != ValidationStatus::Pass {
        if let Some(action) = &result.fix_action {
            println!(
                "      {} {}",
                "💡".with(Color::Blue),
                format!("Can fix: {action}").with(Color::Blue)
            );
        }
    }
}

fn print_summary(summary: &DoctorSummary, fix_mode: bool) {
    println!();
    println!("{}", "═".repeat(60).with(Color::DarkCyan));
    println!(
        "  {} {}",
        "📊".with(Color::Cyan),
        "Summary".with(Color::Cyan).attribute(Attribute::Bold)
    );
    println!("{}", "─".repeat(60).with(Color::DarkGrey));

    let pass_str = format!("{} passed", summary.passed);
    let warn_str = format!("{} warnings", summary.warnings);
    let error_str = format!("{} errors", summary.errors);

    println!(
        "  {} {} {} {} {}",
        pass_str.with(Color::Green),
        "│".with(Color::DarkGrey),
        warn_str.with(Color::Yellow),
        "│".with(Color::DarkGrey),
        error_str.with(Color::Red)
    );

    println!(
        "  ⏱️  Total time: {}",
        format_duration(Duration::from_millis(summary.total_duration_ms)).with(Color::DarkGrey)
    );

    if fix_mode && summary.fixed > 0 {
        println!(
            "  {} {}",
            "🔧".with(Color::Green),
            format!("{} issues auto-fixed", summary.fixed).with(Color::Green)
        );
    } else if summary.fixable > 0 {
        println!(
            "  {} {}",
            "💡".with(Color::Blue),
            format!("{} issues can be auto-fixed with --fix", summary.fixable).with(Color::Blue)
        );
    }

    println!("{}", "═".repeat(60).with(Color::DarkCyan));

    // Final verdict
    if summary.errors > 0 {
        println!(
            "\n  ❌ {}",
            "Some issues need attention"
                .with(Color::Red)
                .attribute(Attribute::Bold)
        );
    } else if summary.warnings > 0 {
        println!(
            "\n  ⚠️  {}",
            "All good with some warnings"
                .with(Color::Yellow)
                .attribute(Attribute::Bold)
        );
    } else {
        println!(
            "\n  ✅ {}",
            "Everything looks healthy!"
                .with(Color::Green)
                .attribute(Attribute::Bold)
        );
    }
    println!();
}

// ============================================================================
// Doctor Implementation
// ============================================================================

impl Doctor {
    #[must_use]
    pub fn new(config: Config, options: DoctorOptions) -> Self {
        Self {
            config,
            options,
            results: Vec::new(),
            start_time: Instant::now(),
        }
    }

    pub fn run_diagnostics(&mut self) -> Result<DoctorReport> {
        if !self.options.json_output {
            print_header("DotState Doctor");
            println!(
                "  {} Running comprehensive diagnostics...",
                "🔍".with(Color::Cyan)
            );
            io::stdout().flush()?;
        }

        // Run all checks by category
        for (i, category) in CATEGORIES.iter().enumerate() {
            if !self.options.json_output {
                print_category_header(category, i, CATEGORIES.len());
            }

            match category.name {
                "Environment" => self.check_environment()?,
                "Configuration" => self.check_configuration()?,
                "Repository" => self.check_repository()?,
                "Profiles" => self.check_profiles()?,
                "Symlinks" => self.check_symlinks()?,
                "Backups" => self.check_backups()?,
                "Filesystem" => self.check_filesystem()?,
                _ => {}
            }
        }

        // Apply fixes if requested
        if self.options.fix_mode {
            self.apply_fixes()?;
        }

        // Calculate summary
        let summary = self.calculate_summary();

        if !self.options.json_output {
            print_summary(&summary, self.options.fix_mode);
        }

        Ok(DoctorReport {
            version: env!("CARGO_PKG_VERSION").to_string(),
            timestamp: chrono::Local::now().to_rfc3339(),
            results: self.results.clone(),
            summary,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn add_result(
        &mut self,
        category: &str,
        check_name: &str,
        message: &str,
        status: ValidationStatus,
        fix_action: Option<&str>,
        details: Option<Vec<String>>,
        start_time: Instant,
    ) {
        let result = ValidationResult {
            category: category.to_string(),
            check_name: check_name.to_string(),
            message: message.to_string(),
            status: status.clone(),
            fixable: fix_action.is_some(),
            fix_action: fix_action.map(std::string::ToString::to_string),
            details,
            duration_ms: start_time.elapsed().as_millis() as u64,
        };

        if !self.options.json_output {
            print_check_result(&result, self.options.verbose);
        }

        self.results.push(result);
    }

    fn calculate_summary(&self) -> DoctorSummary {
        let passed = self
            .results
            .iter()
            .filter(|r| matches!(r.status, ValidationStatus::Pass))
            .count();
        let warnings = self
            .results
            .iter()
            .filter(|r| matches!(r.status, ValidationStatus::Warning))
            .count();
        let errors = self
            .results
            .iter()
            .filter(|r| matches!(r.status, ValidationStatus::Error))
            .count();
        let fixable = self
            .results
            .iter()
            .filter(|r| r.fixable && r.status != ValidationStatus::Pass)
            .count();
        let fixed = if self.options.fix_mode { fixable } else { 0 };

        DoctorSummary {
            total_checks: self.results.len(),
            passed,
            warnings,
            errors,
            fixable,
            fixed,
            total_duration_ms: self.start_time.elapsed().as_millis() as u64,
        }
    }

    // ========================================================================
    // Environment Checks
    // ========================================================================

    fn check_environment(&mut self) -> Result<()> {
        // Check for updates
        self.check_version()?;

        // Check Git version
        let start = Instant::now();
        let git_output = Command::new("git").arg("--version").output();

        match git_output {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                let version = version.trim();
                self.add_result(
                    "Environment",
                    "git_version",
                    &format!("Git installed: {version}"),
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            }
            _ => {
                self.add_result(
                    "Environment",
                    "git_version",
                    "Git not found in PATH",
                    ValidationStatus::Error,
                    None,
                    Some(vec!["Git is required for DotState to function".to_string()]),
                    start,
                );
            }
        }

        // Check shell and home directory
        self.check_shell_and_home()?;

        Ok(())
    }

    fn check_version(&mut self) -> Result<()> {
        use crate::version_check::{check_for_updates_with_result, current_version};

        let start = Instant::now();
        let current = current_version();

        match check_for_updates_with_result() {
            Ok(Some(update_info)) => {
                self.add_result(
                    "Environment",
                    "version",
                    &format!(
                        "Update available: {} → {}",
                        update_info.current_version, update_info.latest_version
                    ),
                    ValidationStatus::Warning,
                    None,
                    Some(vec![
                        format!("Run 'dotstate upgrade' to update"),
                        format!("Release notes: {}", update_info.release_url),
                    ]),
                    start,
                );
            }
            Ok(None) => {
                self.add_result(
                    "Environment",
                    "version",
                    &format!("DotState {current} (latest)"),
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            }
            Err(e) => {
                // Don't fail the doctor just because we couldn't check for updates
                self.add_result(
                    "Environment",
                    "version",
                    &format!("DotState {current} (update check failed)"),
                    ValidationStatus::Pass,
                    None,
                    if self.options.verbose {
                        Some(vec![format!("Error: {}", e)])
                    } else {
                        None
                    },
                    start,
                );
            }
        }

        Ok(())
    }

    fn check_shell_and_home(&mut self) -> Result<()> {
        // Check shell
        let start = Instant::now();
        if let Ok(shell) = std::env::var("SHELL") {
            self.add_result(
                "Environment",
                "shell",
                &format!("Default shell: {shell}"),
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        }

        // Check home directory
        let start = Instant::now();
        if let Some(home) = dirs::home_dir() {
            if home.exists() {
                self.add_result(
                    "Environment",
                    "home_dir",
                    &format!("Home directory: {}", home.display()),
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            } else {
                self.add_result(
                    "Environment",
                    "home_dir",
                    "Home directory does not exist",
                    ValidationStatus::Error,
                    None,
                    None,
                    start,
                );
            }
        }

        Ok(())
    }

    // ========================================================================
    // Configuration Checks
    // ========================================================================

    fn check_configuration(&mut self) -> Result<()> {
        let config_path = crate::utils::get_config_path();

        // Check config file exists
        let start = Instant::now();
        if config_path.exists() {
            self.add_result(
                "Configuration",
                "config_file",
                "Configuration file exists",
                ValidationStatus::Pass,
                None,
                if self.options.verbose {
                    Some(vec![format!("Path: {}", config_path.display())])
                } else {
                    None
                },
                start,
            );
        } else {
            self.add_result(
                "Configuration",
                "config_file",
                "Configuration file missing",
                ValidationStatus::Error,
                None,
                Some(vec![format!("Expected at: {}", config_path.display())]),
                start,
            );
        }

        // Check repository path
        let start = Instant::now();
        if self.config.repo_path.exists() {
            self.add_result(
                "Configuration",
                "repo_path",
                "Repository path exists",
                ValidationStatus::Pass,
                None,
                if self.options.verbose {
                    Some(vec![format!("Path: {}", self.config.repo_path.display())])
                } else {
                    None
                },
                start,
            );
        } else {
            self.add_result(
                "Configuration",
                "repo_path",
                &format!("Repository path not found: {:?}", self.config.repo_path),
                ValidationStatus::Error,
                None,
                None,
                start,
            );
        }

        // Check active profile is set
        let start = Instant::now();
        if self.config.active_profile.is_empty() {
            self.add_result(
                "Configuration",
                "active_profile",
                "No active profile set",
                ValidationStatus::Warning,
                None,
                Some(vec!["Run 'dotstate' to select a profile".to_string()]),
                start,
            );
        } else {
            self.add_result(
                "Configuration",
                "active_profile",
                &format!("Active profile: '{}'", self.config.active_profile),
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        }

        Ok(())
    }

    // ========================================================================
    // Repository Checks
    // ========================================================================

    fn check_repository(&mut self) -> Result<()> {
        // Check if valid git repo
        let start = Instant::now();
        if crate::utils::is_git_repo(&self.config.repo_path) {
            self.add_result(
                "Repository",
                "git_repo",
                "Valid git repository",
                ValidationStatus::Pass,
                None,
                None,
                start,
            );

            // Check remote
            self.check_git_remote()?;

            // Check working tree status
            self.check_git_status()?;

            // Check branch status
            self.check_git_branch()?;
        } else {
            self.add_result(
                "Repository",
                "git_repo",
                "Not a git repository",
                ValidationStatus::Warning,
                None,
                Some(vec!["Git sync features will not work".to_string()]),
                start,
            );
        }

        Ok(())
    }

    fn check_git_remote(&mut self) -> Result<()> {
        let start = Instant::now();
        let output = Command::new("git")
            .args(["remote", "-v"])
            .current_dir(&self.config.repo_path)
            .output();

        match output {
            Ok(output) if output.status.success() => {
                let remote_str = String::from_utf8_lossy(&output.stdout);
                if remote_str.trim().is_empty() {
                    self.add_result(
                        "Repository",
                        "git_remote",
                        "No remote configured",
                        ValidationStatus::Warning,
                        None,
                        Some(vec!["Changes won't sync to a remote server".to_string()]),
                        start,
                    );
                } else {
                    let origin = remote_str
                        .lines()
                        .find(|l| l.contains("origin") && l.contains("(fetch)"))
                        .and_then(|l| l.split_whitespace().nth(1));

                    if let Some(url) = origin {
                        // Redact any tokens/credentials from the URL
                        let safe_url = crate::git::redact_credentials(url);
                        self.add_result(
                            "Repository",
                            "git_remote",
                            &format!("Remote configured: {safe_url}"),
                            ValidationStatus::Pass,
                            None,
                            None,
                            start,
                        );
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn check_git_status(&mut self) -> Result<()> {
        let start = Instant::now();
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&self.config.repo_path)
            .output();

        if let Ok(output) = output {
            let status_str = String::from_utf8_lossy(&output.stdout);
            let changes: Vec<&str> = status_str.lines().collect();

            if changes.is_empty() {
                self.add_result(
                    "Repository",
                    "git_status",
                    "Working tree clean",
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            } else {
                let details = if self.options.verbose {
                    Some(
                        changes
                            .iter()
                            .take(5)
                            .map(std::string::ToString::to_string)
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                };

                self.add_result(
                    "Repository",
                    "git_status",
                    &format!("{} uncommitted changes", changes.len()),
                    ValidationStatus::Warning,
                    None,
                    details,
                    start,
                );
            }
        }

        Ok(())
    }

    fn check_git_branch(&mut self) -> Result<()> {
        let start = Instant::now();

        // Get current branch
        let branch_output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.config.repo_path)
            .output();

        if let Ok(output) = branch_output {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

            if !branch.is_empty() {
                // Check ahead/behind
                let status_output = Command::new("git")
                    .args([
                        "rev-list",
                        "--left-right",
                        "--count",
                        &format!("origin/{branch}...HEAD"),
                    ])
                    .current_dir(&self.config.repo_path)
                    .output();

                if let Ok(status) = status_output {
                    let counts = String::from_utf8_lossy(&status.stdout);
                    let parts: Vec<&str> = counts.split_whitespace().collect();

                    if parts.len() == 2 {
                        let behind: i32 = parts[0].parse().unwrap_or(0);
                        let ahead: i32 = parts[1].parse().unwrap_or(0);

                        let message = match (ahead, behind) {
                            (0, 0) => format!("Branch '{branch}' up to date with remote"),
                            (a, 0) => {
                                format!("Branch '{branch}' {a} commit(s) ahead of remote")
                            }
                            (0, b) => format!("Branch '{branch}' {b} commit(s) behind remote"),
                            (a, b) => {
                                format!("Branch '{branch}' {a} ahead, {b} behind remote")
                            }
                        };

                        let status = if behind > 0 {
                            ValidationStatus::Warning
                        } else {
                            ValidationStatus::Pass
                        };

                        self.add_result(
                            "Repository",
                            "git_branch",
                            &message,
                            status,
                            None,
                            None,
                            start,
                        );
                    } else {
                        self.add_result(
                            "Repository",
                            "git_branch",
                            &format!("Current branch: {branch}"),
                            ValidationStatus::Pass,
                            None,
                            None,
                            start,
                        );
                    }
                } else {
                    self.add_result(
                        "Repository",
                        "git_branch",
                        &format!("Current branch: {branch} (no upstream)"),
                        ValidationStatus::Pass,
                        None,
                        None,
                        start,
                    );
                }
            }
        }

        Ok(())
    }

    // ========================================================================
    // Profile Checks
    // ========================================================================

    fn check_profiles(&mut self) -> Result<()> {
        let start = Instant::now();

        match ProfileManifest::load(&self.config.repo_path) {
            Ok(manifest) => {
                self.add_result(
                    "Profiles",
                    "manifest",
                    &format!(
                        "Manifest loaded: {} profiles, {} common files",
                        manifest.profiles.len(),
                        manifest.common.synced_files.len()
                    ),
                    ValidationStatus::Pass,
                    None,
                    if self.options.verbose {
                        Some(
                            manifest
                                .profiles
                                .iter()
                                .map(|p| format!("  {} ({} files)", p.name, p.synced_files.len()))
                                .collect(),
                        )
                    } else {
                        None
                    },
                    start,
                );

                // Check active profile exists
                if !self.config.active_profile.is_empty() {
                    let start = Instant::now();
                    if let Some(profile) = manifest
                        .profiles
                        .iter()
                        .find(|p| p.name == self.config.active_profile)
                    {
                        self.add_result(
                            "Profiles",
                            "active_profile_exists",
                            "Active profile exists in manifest",
                            ValidationStatus::Pass,
                            None,
                            None,
                            start,
                        );

                        // Check profile files exist in storage
                        self.check_profile_files(&profile.name, &profile.synced_files)?;
                    } else {
                        self.add_result(
                            "Profiles",
                            "active_profile_exists",
                            &format!(
                                "Active profile '{}' not found in manifest",
                                self.config.active_profile
                            ),
                            ValidationStatus::Error,
                            None,
                            None,
                            start,
                        );
                    }
                }

                // Check common files exist
                if !manifest.common.synced_files.is_empty() {
                    self.check_common_files(&manifest.common.synced_files)?;
                }
            }
            Err(e) => {
                self.add_result(
                    "Profiles",
                    "manifest",
                    &format!("Failed to load manifest: {e}"),
                    ValidationStatus::Error,
                    Some("Rebuild manifest"),
                    None,
                    start,
                );
            }
        }

        Ok(())
    }

    fn check_profile_files(&mut self, profile_name: &str, files: &[String]) -> Result<()> {
        let start = Instant::now();
        let profile_path = self.config.repo_path.join(profile_name);
        let mut missing = Vec::new();

        for file in files {
            if !profile_path.join(file).exists() {
                missing.push(file.clone());
            }
        }

        if missing.is_empty() {
            self.add_result(
                "Profiles",
                "profile_files",
                &format!("All {} profile files exist in storage", files.len()),
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        } else {
            self.add_result(
                "Profiles",
                "profile_files",
                &format!("{} profile files missing from storage", missing.len()),
                ValidationStatus::Error,
                None,
                Some(missing.iter().take(5).cloned().collect()),
                start,
            );
        }

        Ok(())
    }

    fn check_common_files(&mut self, files: &[String]) -> Result<()> {
        let start = Instant::now();
        let common_path = self.config.repo_path.join("common");
        let mut missing = Vec::new();

        for file in files {
            if !common_path.join(file).exists() {
                missing.push(file.clone());
            }
        }

        if missing.is_empty() {
            self.add_result(
                "Profiles",
                "common_files",
                &format!("All {} common files exist in storage", files.len()),
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        } else {
            self.add_result(
                "Profiles",
                "common_files",
                &format!("{} common files missing from storage", missing.len()),
                ValidationStatus::Error,
                None,
                Some(missing.iter().take(5).cloned().collect()),
                start,
            );
        }

        Ok(())
    }

    // ========================================================================
    // Symlink Checks
    // ========================================================================

    fn check_symlinks(&mut self) -> Result<()> {
        // Check activation status consistency
        self.check_activation_status()?;

        if self.config.profile_activated {
            // Check tracking
            self.check_symlink_tracking()?;

            // Check symlink validity
            self.check_symlink_validity()?;
        }

        Ok(())
    }

    fn check_activation_status(&mut self) -> Result<()> {
        let start = Instant::now();

        if self.config.profile_activated {
            let symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;

            if symlink_mgr.tracking.active_profile == self.config.active_profile {
                self.add_result(
                    "Symlinks",
                    "activation_status",
                    &format!("Profile '{}' is active", self.config.active_profile),
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            } else if symlink_mgr.tracking.active_profile.is_empty() {
                self.add_result(
                    "Symlinks",
                    "activation_status",
                    "Config says active, but tracking says inactive",
                    ValidationStatus::Warning,
                    Some("Sync activation state"),
                    None,
                    start,
                );
            } else {
                self.add_result(
                    "Symlinks",
                    "activation_status",
                    &format!(
                        "Profile mismatch: config='{}', tracking='{}'",
                        self.config.active_profile, symlink_mgr.tracking.active_profile
                    ),
                    ValidationStatus::Warning,
                    Some("Sync activation state"),
                    None,
                    start,
                );
            }
        } else {
            self.add_result(
                "Symlinks",
                "activation_status",
                "No profile currently active",
                ValidationStatus::Pass,
                None,
                Some(vec![
                    "Run 'dotstate activate' to enable symlinks".to_string()
                ]),
                start,
            );
        }

        Ok(())
    }

    fn check_symlink_tracking(&mut self) -> Result<()> {
        let start = Instant::now();
        let symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;

        if symlink_mgr.tracking.symlinks.is_empty() {
            self.add_result(
                "Symlinks",
                "tracking",
                "No symlinks tracked (profile may need re-activation)",
                ValidationStatus::Warning,
                Some("Re-activate profile"),
                None,
                start,
            );
            return Ok(());
        }

        self.add_result(
            "Symlinks",
            "tracking",
            &format!("{} symlinks tracked", symlink_mgr.tracking.symlinks.len()),
            ValidationStatus::Pass,
            None,
            None,
            start,
        );

        // Check for orphaned tracking entries
        let start = Instant::now();
        let mut orphaned = 0;
        for tracked in &symlink_mgr.tracking.symlinks {
            if !tracked.target.exists() && tracked.target.symlink_metadata().is_err() {
                orphaned += 1;
            }
        }

        if orphaned > 0 {
            self.add_result(
                "Symlinks",
                "orphaned",
                &format!("{orphaned} tracked symlinks missing from disk"),
                ValidationStatus::Warning,
                Some("Clean up missing symlinks"),
                None,
                start,
            );
        } else {
            self.add_result(
                "Symlinks",
                "orphaned",
                "All tracked symlinks exist on disk",
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        }

        // Check for untracked expected files
        self.check_untracked_files(&symlink_mgr)?;

        Ok(())
    }

    fn check_untracked_files(&mut self, symlink_mgr: &SymlinkManager) -> Result<()> {
        let start = Instant::now();

        if let Ok(manifest) = ProfileManifest::load(&self.config.repo_path) {
            let mut expected = HashSet::new();

            // Add active profile files
            if let Some(profile) = manifest
                .profiles
                .iter()
                .find(|p| p.name == self.config.active_profile)
            {
                for file in &profile.synced_files {
                    expected.insert(file.clone());
                }
            }

            // Add common files
            for file in &manifest.common.synced_files {
                expected.insert(file.clone());
            }

            let mut untracked = Vec::new();
            for exp in expected {
                let is_tracked = symlink_mgr
                    .tracking
                    .symlinks
                    .iter()
                    .any(|s| s.source.ends_with(&exp));
                if !is_tracked {
                    untracked.push(exp);
                }
            }

            if untracked.is_empty() {
                self.add_result(
                    "Symlinks",
                    "coverage",
                    "All expected files are tracked",
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            } else {
                self.add_result(
                    "Symlinks",
                    "coverage",
                    &format!("{} expected files not tracked", untracked.len()),
                    ValidationStatus::Error,
                    Some("Re-activate profile"),
                    Some(untracked.iter().take(5).cloned().collect()),
                    start,
                );
            }
        }

        Ok(())
    }

    fn check_symlink_validity(&mut self) -> Result<()> {
        let start = Instant::now();
        let symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;

        let mut invalid = Vec::new();
        let mut broken = Vec::new();

        for tracked in &symlink_mgr.tracking.symlinks {
            // Check if target exists and is a symlink
            if let Ok(metadata) = tracked.target.symlink_metadata() {
                if metadata.is_symlink() {
                    // Verify symlink points to correct source
                    if let Ok(link_target) = fs::read_link(&tracked.target) {
                        if link_target != tracked.source {
                            invalid.push(format!(
                                "{} -> {} (expected {})",
                                tracked.target.display(),
                                link_target.display(),
                                tracked.source.display()
                            ));
                        }
                    }
                } else {
                    // File exists but is not a symlink
                    invalid.push(format!(
                        "{} exists but is not a symlink",
                        tracked.target.display()
                    ));
                }
            } else {
                // Symlink doesn't exist
                broken.push(tracked.target.display().to_string());
            }
        }

        if invalid.is_empty() && broken.is_empty() {
            self.add_result(
                "Symlinks",
                "validity",
                "All symlinks are valid and point to correct targets",
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        } else {
            if !invalid.is_empty() {
                self.add_result(
                    "Symlinks",
                    "invalid",
                    &format!("{} symlinks point to wrong targets", invalid.len()),
                    ValidationStatus::Error,
                    Some("Re-activate profile"),
                    if self.options.verbose {
                        Some(invalid.iter().take(3).cloned().collect())
                    } else {
                        None
                    },
                    start,
                );
            }

            if !broken.is_empty() {
                let start = Instant::now();
                self.add_result(
                    "Symlinks",
                    "broken",
                    &format!("{} symlinks are broken/missing", broken.len()),
                    ValidationStatus::Warning,
                    Some("Re-activate profile"),
                    if self.options.verbose {
                        Some(broken.iter().take(3).cloned().collect())
                    } else {
                        None
                    },
                    start,
                );
            }
        }

        Ok(())
    }

    // ========================================================================
    // Backup Checks
    // ========================================================================

    fn check_backups(&mut self) -> Result<()> {
        // First check if backups are enabled in config
        let start = Instant::now();
        if self.config.backup_enabled {
            self.add_result(
                "Backups",
                "backup_enabled",
                "Backups are enabled",
                ValidationStatus::Pass,
                None,
                None,
                start,
            );
        } else {
            self.add_result(
                "Backups",
                "backup_enabled",
                "Backups are disabled in settings",
                ValidationStatus::Warning,
                None,
                Some(vec![
                    "Enable backups in settings to protect your files".to_string()
                ]),
                start,
            );
        }

        // Check backup directory - it's in ~/.dotstate-backups
        let start = Instant::now();
        let backup_dir = dirs::home_dir()
            .map(|h| h.join(".dotstate-backups"))
            .unwrap_or_default();

        if !backup_dir.exists() {
            let message = if self.config.backup_enabled {
                "No backup directory yet (will be created on first backup)"
            } else {
                "No backup directory (backups are disabled)"
            };
            self.add_result(
                "Backups",
                "backup_dir",
                message,
                ValidationStatus::Pass,
                None,
                if self.options.verbose {
                    Some(vec![format!("Expected at: {}", backup_dir.display())])
                } else {
                    None
                },
                start,
            );
            return Ok(());
        }

        // Count backup sessions and total size
        let mut session_count = 0;
        let mut total_size: u64 = 0;

        if let Ok(entries) = fs::read_dir(&backup_dir) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_dir() {
                        session_count += 1;
                        // Calculate size of this session
                        total_size += Self::dir_size(&entry.path());
                    }
                }
            }
        }

        let size_str = Self::format_size(total_size);

        if session_count > 0 {
            self.add_result(
                "Backups",
                "backup_files",
                &format!("{session_count} backup session(s) ({size_str} total)"),
                ValidationStatus::Pass,
                None,
                if self.options.verbose {
                    Some(vec![format!("Location: {}", backup_dir.display())])
                } else {
                    None
                },
                start,
            );

            // Warn if backups are large
            if total_size > 100 * 1024 * 1024 {
                // > 100MB
                let start = Instant::now();
                self.add_result(
                    "Backups",
                    "backup_size",
                    &format!("Backup directory is large ({size_str})"),
                    ValidationStatus::Warning,
                    None,
                    Some(vec![
                        "Consider cleaning old backups to save space".to_string(),
                        format!("Location: {}", backup_dir.display()),
                    ]),
                    start,
                );
            }
        } else {
            self.add_result(
                "Backups",
                "backup_files",
                "Backup directory exists but is empty",
                ValidationStatus::Pass,
                None,
                if self.options.verbose {
                    Some(vec![format!("Location: {}", backup_dir.display())])
                } else {
                    None
                },
                start,
            );
        }

        Ok(())
    }

    /// Calculate directory size recursively
    fn dir_size(path: &std::path::Path) -> u64 {
        let mut size = 0;
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(meta) = entry.metadata() {
                    if meta.is_file() {
                        size += meta.len();
                    } else if meta.is_dir() {
                        size += Self::dir_size(&entry.path());
                    }
                }
            }
        }
        size
    }

    /// Format byte size to human readable string
    fn format_size(bytes: u64) -> String {
        if bytes < 1024 {
            format!("{bytes} B")
        } else if bytes < 1024 * 1024 {
            format!("{:.1} KB", bytes as f64 / 1024.0)
        } else if bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }

    // ========================================================================
    // Filesystem Checks
    // ========================================================================

    fn check_filesystem(&mut self) -> Result<()> {
        // Check write permissions
        self.check_write_permissions()?;

        // Check disk space
        self.check_disk_space()?;

        Ok(())
    }

    fn check_write_permissions(&mut self) -> Result<()> {
        let start = Instant::now();
        let test_file = self.config.repo_path.join(".doctor_write_test");

        match fs::write(&test_file, "test") {
            Ok(()) => {
                let _ = fs::remove_file(&test_file);
                self.add_result(
                    "Filesystem",
                    "write_permission",
                    "Repository is writable",
                    ValidationStatus::Pass,
                    None,
                    None,
                    start,
                );
            }
            Err(e) => {
                self.add_result(
                    "Filesystem",
                    "write_permission",
                    &format!("Repository not writable: {e}"),
                    ValidationStatus::Error,
                    None,
                    None,
                    start,
                );
            }
        }

        // Check home directory write permissions
        let start = Instant::now();
        if let Some(home) = dirs::home_dir() {
            let test_file = home.join(".dotstate_doctor_test");
            match fs::write(&test_file, "test") {
                Ok(()) => {
                    let _ = fs::remove_file(&test_file);
                    self.add_result(
                        "Filesystem",
                        "home_writable",
                        "Home directory is writable",
                        ValidationStatus::Pass,
                        None,
                        None,
                        start,
                    );
                }
                Err(e) => {
                    self.add_result(
                        "Filesystem",
                        "home_writable",
                        &format!("Home directory not writable: {e}"),
                        ValidationStatus::Error,
                        None,
                        None,
                        start,
                    );
                }
            }
        }

        Ok(())
    }

    fn check_disk_space(&mut self) -> Result<()> {
        let start = Instant::now();

        // Use df command to check disk space
        let output = Command::new("df")
            .args(["-h", self.config.repo_path.to_str().unwrap_or(".")])
            .output();

        if let Ok(output) = output {
            let df_output = String::from_utf8_lossy(&output.stdout);

            // Parse the output (skip header, get usage percentage)
            if let Some(line) = df_output.lines().nth(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let usage = parts[4].trim_end_matches('%');
                    if let Ok(pct) = usage.parse::<i32>() {
                        let available = parts.get(3).unwrap_or(&"unknown");

                        if pct > 90 {
                            self.add_result(
                                "Filesystem",
                                "disk_space",
                                &format!("Disk {pct}% full - {available} available"),
                                ValidationStatus::Error,
                                None,
                                Some(vec!["Low disk space may cause sync failures".to_string()]),
                                start,
                            );
                        } else if pct > 80 {
                            self.add_result(
                                "Filesystem",
                                "disk_space",
                                &format!("Disk {pct}% full - {available} available"),
                                ValidationStatus::Warning,
                                None,
                                None,
                                start,
                            );
                        } else {
                            self.add_result(
                                "Filesystem",
                                "disk_space",
                                &format!("Disk space OK - {available} available ({pct}% used)"),
                                ValidationStatus::Pass,
                                None,
                                None,
                                start,
                            );
                        }
                        return Ok(());
                    }
                }
            }
        }

        // Fallback if df parsing fails
        self.add_result(
            "Filesystem",
            "disk_space",
            "Could not determine disk space",
            ValidationStatus::Pass,
            None,
            None,
            start,
        );

        Ok(())
    }

    // ========================================================================
    // Fix Implementation
    // ========================================================================

    fn apply_fixes(&mut self) -> Result<()> {
        let fixable: Vec<ValidationResult> = self
            .results
            .iter()
            .filter(|r| r.fixable && r.status != ValidationStatus::Pass)
            .cloned()
            .collect();

        if fixable.is_empty() {
            return Ok(());
        }

        if !self.options.json_output {
            println!();
            println!(
                "{} {} {}",
                "🔧".with(Color::Cyan),
                "Applying fixes"
                    .with(Color::Cyan)
                    .attribute(Attribute::Bold),
                format!("({} issues)", fixable.len()).with(Color::DarkGrey)
            );
            println!("{}", "─".repeat(50).with(Color::DarkGrey));
        }

        for issue in fixable {
            if let Some(action) = &issue.fix_action {
                let success = self.apply_single_fix(action)?;
                if !self.options.json_output {
                    let icon = if success {
                        "✓".with(Color::Green).to_string()
                    } else {
                        "✗".with(Color::Red).to_string()
                    };
                    println!("  {icon} {action}");
                }
            }
        }

        Ok(())
    }

    fn apply_single_fix(&mut self, action: &str) -> Result<bool> {
        match action {
            "Sync activation state" => {
                let mut config = self.config.clone();
                config.profile_activated = false;
                config.save(&crate::utils::get_config_path())?;
                Ok(true)
            }
            "Clean up missing symlinks" => {
                let mut symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;
                symlink_mgr
                    .tracking
                    .symlinks
                    .retain(|s| s.target.exists() || s.target.symlink_metadata().is_ok());
                symlink_mgr.save_tracking()?;
                Ok(true)
            }
            "Re-activate profile" => {
                use crate::services::ProfileService;

                if self.config.active_profile.is_empty() {
                    Ok(false)
                } else {
                    ProfileService::activate_profile(
                        &self.config.repo_path,
                        &self.config.active_profile,
                        false,
                    )?;
                    Ok(true)
                }
            }
            "Rebuild manifest" => {
                // Re-scan filesystem and rebuild manifest
                let _ = ProfileManifest::load_or_backfill(&self.config.repo_path)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

// Keep the old interface for compatibility (will be removed later)
impl Doctor {
    #[allow(dead_code)]
    #[must_use]
    pub fn new_legacy(config: Config, fix_mode: bool) -> Self {
        Self::new(
            config,
            DoctorOptions {
                fix_mode,
                verbose: false,
                json_output: false,
            },
        )
    }
}
