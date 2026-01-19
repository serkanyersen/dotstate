use anyhow::Result;
use std::fs;
use std::process::Command;

use crate::config::Config;
use crate::utils::{ProfileManifest, SymlinkManager};

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationStatus {
    Pass,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub category: String,
    pub message: String,
    pub status: ValidationStatus,
    pub fixable: bool,
    pub fix_action: Option<String>,
}

pub struct Doctor {
    config: Config,
    results: Vec<ValidationResult>,
    fix_mode: bool,
}

impl Doctor {
    pub fn new(config: Config, fix_mode: bool) -> Self {
        Self {
            config,
            results: Vec::new(),
            fix_mode,
        }
    }

    pub fn run_diagnostics(&mut self) -> Result<Vec<ValidationResult>> {
        println!("\n🔍 Running diagnostics...\n");

        self.check_configuration()?;
        self.check_activation()?;
        self.check_manifest()?;
        self.check_tracking()?;
        self.check_git()?;
        self.check_permissions()?;

        if self.fix_mode {
            self.fix_issues()?;
        }

        Ok(self.results.clone())
    }

    fn fix_issues(&mut self) -> Result<()> {
         // Filter for fixable issues
         let fixable: Vec<ValidationResult> = self.results.iter()
             .filter(|r| r.fixable && r.status != ValidationStatus::Pass)
             .cloned()
             .collect();

         if fixable.is_empty() {
             return Ok(());
         }

         println!("\n🔧 Attempting to fix {} issues...", fixable.len());

         for issue in fixable {
             if let Some(action) = &issue.fix_action {
                 match action.as_str() {
                     "Sync activation state" => {
                        // If config says active but tracking doesn't, we trust tracking (inactive)
                         // OR we try to activate?
                         // Safer to just mark config as inactive to match reality
                         println!("   Applying fix: Sync activation state...");
                         let mut config = self.config.clone();
                         config.profile_activated = false;
                         config.save(&crate::utils::get_config_path())?;
                         println!("   ✅ Config updated to match tracking state (inactive)");
                     },
                     "Clean up missing symlinks from tracking" => {
                         println!("   Applying fix: Cleaning up missing symlinks...");
                         let mut symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;
                         // Reload tracking
                         // Only keep symlinks that exist or point to valid locations?
                         // Ideally SymlinkManager would have a 'prune' method.
                         // For now, we manually filter.
                         symlink_mgr.tracking.symlinks.retain(|s| s.target.exists() || s.target.symlink_metadata().is_ok());
                         symlink_mgr.save_tracking()?;
                         println!("   ✅ Removed zombie entries from tracking file");
                     },
                     _ => {
                         println!("   ⚠️  Fix not implemented for: {}", action);
                     }
                 }
             }
         }

         Ok(())
    }

    fn add_result(
        &mut self,
        category: &str,
        message: &str,
        status: ValidationStatus,
        fix_action: Option<String>,
    ) {
        let result = ValidationResult {
            category: category.to_string(),
            message: message.to_string(),
            status: status.clone(),
            fixable: fix_action.is_some(),
            fix_action,
        };

        match status {
            ValidationStatus::Pass => println!("   ✅ {}", message),
            ValidationStatus::Warning => println!("   ⚠️  {}", message),
            ValidationStatus::Error => println!("   ❌ {}", message),
        }

        self.results.push(result);
    }

    fn check_configuration(&mut self) -> Result<()> {
        println!("[1/6] Configuration...");

        let config_path = crate::utils::get_config_path();
        if config_path.exists() {
            self.add_result("Config", "Configuration file exists", ValidationStatus::Pass, None);
        } else {
            self.add_result(
                "Config",
                "Configuration file missing",
                ValidationStatus::Error,
                None
            );
        }

        if self.config.repo_path.exists() {
            self.add_result("Config", "Repository path exists", ValidationStatus::Pass, None);
        } else {
            self.add_result(
                "Config",
                &format!("Repository path not found: {:?}", self.config.repo_path),
                ValidationStatus::Error,
                None // User intervention required usually
            );
        }

        Ok(())
    }

    fn check_activation(&mut self) -> Result<()> {
        println!("\n[2/6] Activation Status...");

        if self.config.profile_activated {
            self.add_result(
                "Activation",
                &format!("Profile '{}' is marked as active in config", self.config.active_profile),
                ValidationStatus::Pass,
                None
            );

            // Check if tracking file agrees
            let symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;
            if symlink_mgr.tracking.active_profile == self.config.active_profile {
                self.add_result(
                    "Activation",
                    "Tracking file matches active profile",
                    ValidationStatus::Pass,
                    None
                );
            } else if symlink_mgr.tracking.active_profile.is_empty() {
                self.add_result(
                    "Activation",
                    "Config says active, but tracking file says inactive",
                    ValidationStatus::Warning,
                    Some("Sync activation state".to_string())
                );
            } else {
                self.add_result(
                    "Activation",
                    &format!("Profile mismatch: Config='{}', Tracking='{}'",
                        self.config.active_profile, symlink_mgr.tracking.active_profile),
                    ValidationStatus::Warning,
                    None
                );
            }
        } else {
            self.add_result("Activation", "No profile currently active", ValidationStatus::Pass, None);
        }

        Ok(())
    }

    fn check_manifest(&mut self) -> Result<()> {
        println!("\n[3/6] Manifest Integrity...");

        let manifest_result = ProfileManifest::load(&self.config.repo_path);
        match manifest_result {
            Ok(manifest) => {
                self.add_result(
                    "Manifest",
                    &format!("Manifest loaded ({} profiles, {} common files)",
                        manifest.profiles.len(), manifest.common.synced_files.len()),
                    ValidationStatus::Pass,
                    None
                );

                // Check active profile exists
                if !self.config.active_profile.is_empty() {
                    if let Some(profile) = manifest.profiles.iter().find(|p| p.name == self.config.active_profile) {
                        self.add_result(
                            "Manifest",
                            "Active profile exists in manifest",
                            ValidationStatus::Pass,
                            None
                        );

                        // Check actual files exist in storage
                        let mut missing_files = Vec::new();
                        let profile_path = self.config.repo_path.join(&profile.name);

                        for file in &profile.synced_files {
                            if !profile_path.join(file).exists() {
                                missing_files.push(file.clone());
                            }
                        }

                        if missing_files.is_empty() {
                            self.add_result(
                                "Manifest",
                                &format!("All {} profile files exist in storage", profile.synced_files.len()),
                                ValidationStatus::Pass,
                                None
                            );
                        } else {
                            self.add_result(
                                "Manifest",
                                &format!("{} files missing from storage: {:?}", missing_files.len(), missing_files),
                                ValidationStatus::Error,
                                None
                            );
                        }
                    } else {
                        self.add_result(
                            "Manifest",
                            &format!("Active profile '{}' not found in manifest", self.config.active_profile),
                            ValidationStatus::Error,
                            None
                        );
                    }
                }
            },
            Err(e) => {
                self.add_result(
                    "Manifest",
                    &format!("Failed to load manifest: {}", e),
                    ValidationStatus::Error,
                    None
                );
            }
        }

        Ok(())
    }

    fn check_tracking(&mut self) -> Result<()> {
        println!("\n[4/6] Symlink Tracking...");

        let symlink_mgr = SymlinkManager::new(self.config.repo_path.clone())?;

        if self.config.profile_activated {
            if symlink_mgr.tracking.symlinks.is_empty() {
                self.add_result(
                    "Tracking",
                    "No symlinks tracked specifically, but profile is active",
                    ValidationStatus::Warning,
                    Some("Re-scan symlinks".to_string())
                );
            } else {
                self.add_result(
                    "Tracking",
                    &format!("{} files tracked", symlink_mgr.tracking.symlinks.len()),
                    ValidationStatus::Pass,
                    None
                );

                // Check for orphan symlinks (tracked but not existing)
                let mut missing_symlinks = 0;
                for tracked in &symlink_mgr.tracking.symlinks {
                    if !tracked.target.exists() && tracked.target.symlink_metadata().is_err() {
                        missing_symlinks += 1;
                    }
                }

                if missing_symlinks > 0 {
                    self.add_result(
                        "Tracking",
                        &format!("{} tracked symlinks are missing from disk", missing_symlinks),
                        ValidationStatus::Warning,
                        Some("Clean up missing symlinks from tracking".to_string())
                    );
                } else {
                    self.add_result(
                        "Tracking",
                        "All tracked symlinks exist on disk",
                        ValidationStatus::Pass,
                        None
                    );
                }
            }
        }

        Ok(())
    }

    fn check_git(&mut self) -> Result<()> {
        println!("\n[5/6] Git Repository...");

        if crate::utils::is_git_repo(&self.config.repo_path) {
            self.add_result("Git", "Valid git repository", ValidationStatus::Pass, None);

            // remote check
            let remote_output = Command::new("git")
                .args(&["remote", "-v"])
                .current_dir(&self.config.repo_path)
                .output();

            match remote_output {
                Ok(output) if output.status.success() => {
                    let remote_str = String::from_utf8_lossy(&output.stdout);
                    if remote_str.trim().is_empty() {
                         self.add_result("Git", "No remote configured", ValidationStatus::Warning, None);
                    } else {
                         // Parse origin
                         let origin = remote_str.lines()
                             .find(|l| l.contains("origin") && l.contains("(fetch)"))
                             .map(|l| l.split_whitespace().nth(1).unwrap_or(""));

                         if let Some(url) = origin {
                             self.add_result(
                                 "Git",
                                 &format!("Remote 'origin': {}", url),
                                 ValidationStatus::Pass, None
                            );

                            // Try network check only if connectivity seems available
                            // Skipping actual network call for now to keep doctor fast/safe
                            // Typically verify-remote would be separate
                         }
                    }
                },
                _ => {}
            }

            // Status check
            let status_output = Command::new("git")
                .args(&["status", "--porcelain"])
                .current_dir(&self.config.repo_path)
                .output();

            if let Ok(output) = status_output {
                let status_str = String::from_utf8_lossy(&output.stdout);
                let changes = status_str.lines().count();
                if changes > 0 {
                    self.add_result(
                        "Git",
                        &format!("{} uncommitted changes", changes),
                        ValidationStatus::Warning,
                        None
                    );
                } else {
                    self.add_result("Git", "Working tree clean", ValidationStatus::Pass, None);
                }
            }

        } else {
             self.add_result("Git", "Not a git repository", ValidationStatus::Warning, None);
        }

        Ok(())
    }

    fn check_permissions(&mut self) -> Result<()> {
        println!("\n[6/6] Filesystem Permissions...");

        // Check if we can write to repo
        let test_file = self.config.repo_path.join(".write_test");
        match fs::write(&test_file, "test") {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
                self.add_result("Permissions", "Repository is writable", ValidationStatus::Pass, None);
            },
            Err(e) => {
                self.add_result(
                    "Permissions",
                    &format!("Repository not writable: {}", e),
                    ValidationStatus::Error,
                    None
                );
            }
        }

        Ok(())
    }
}
