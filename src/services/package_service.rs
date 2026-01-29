//! Package service for package management operations.
//!
//! This module provides a service layer for package-related operations,
//! abstracting the details of package management from the UI layer.

use crate::utils::package_installer::PackageInstaller;
use crate::utils::package_manager::PackageManagerImpl;
use crate::utils::profile_manifest::{Package, PackageManager};
use crate::utils::ProfileManifest;
use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

/// Status of a package installation check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCheckStatus {
    /// Package is installed.
    Installed,
    /// Package is not installed.
    NotInstalled,
    /// Error occurred during check.
    Error(String),
    /// Status is unknown (not yet checked).
    Unknown,
}

/// Result of checking a package's installation status.
#[derive(Debug)]
pub struct PackageCheckResult {
    /// The status of the package.
    pub status: PackageCheckStatus,
    /// Whether a fallback method was used to check.
    pub used_fallback: bool,
}

/// Result of validating a package.
#[derive(Debug)]
pub struct PackageValidation {
    /// Whether the package is valid.
    pub is_valid: bool,
    /// Error message if not valid.
    pub error_message: Option<String>,
}

/// Service for package-related operations.
///
/// This service provides a clean interface for package operations without
/// direct dependencies on UI state.
/// Parameters for creating a package.
#[derive(Debug)]
pub struct PackageCreationParams<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub manager: PackageManager,
    pub is_custom: bool,
    pub package_name: &'a str,
    pub binary_name: &'a str,
    pub install_command: &'a str,
    pub existence_check: &'a str,
    pub manager_check: &'a str,
}

pub struct PackageService;

impl PackageService {
    /// Get available package managers on this system.
    ///
    /// # Returns
    ///
    /// List of available package managers.
    #[must_use]
    pub fn get_available_managers() -> Vec<PackageManager> {
        PackageManagerImpl::get_available_managers()
    }

    /// Check if a specific package manager is installed.
    ///
    /// # Arguments
    ///
    /// * `manager` - The package manager to check.
    ///
    /// # Returns
    ///
    /// True if the manager is installed.
    #[must_use]
    pub fn is_manager_installed(manager: &PackageManager) -> bool {
        PackageManagerImpl::is_manager_installed(manager)
    }

    /// Check if a package is installed.
    ///
    /// # Arguments
    ///
    /// * `package` - The package to check.
    ///
    /// # Returns
    ///
    /// Result of the check including status and whether fallback was used.
    pub fn check_package(package: &Package) -> PackageCheckResult {
        match PackageInstaller::check_exists(package) {
            Ok((true, check_cmd, _out)) => {
                // Heuristic: if command is "which ...", used_fallback is false.
                // Otherwise (manager check or custom check), it is true.
                let used_fallback = check_cmd
                    .as_ref()
                    .is_some_and(|cmd| !cmd.starts_with("which "));

                debug!(
                    "Package {} found (used_fallback: {})",
                    package.name, used_fallback
                );
                PackageCheckResult {
                    status: PackageCheckStatus::Installed,
                    used_fallback,
                }
            }
            Ok((false, _, _)) => {
                // Package not found - check if manager is installed
                if PackageManagerImpl::is_manager_installed(&package.manager) {
                    debug!(
                        "Package {} not found (manager {:?} is available)",
                        package.name, package.manager
                    );
                    PackageCheckResult {
                        status: PackageCheckStatus::NotInstalled,
                        used_fallback: false,
                    }
                } else {
                    warn!(
                        "Package {} not found and manager {:?} is not installed",
                        package.name, package.manager
                    );
                    PackageCheckResult {
                        status: PackageCheckStatus::Error(format!(
                            "Package not found and package manager '{:?}' is not installed",
                            package.manager
                        )),
                        used_fallback: false,
                    }
                }
            }
            Err(e) => {
                warn!("Error checking package {}: {}", package.name, e);
                PackageCheckResult {
                    status: PackageCheckStatus::Error(format!("Error checking package: {e}")),
                    used_fallback: false,
                }
            }
        }
    }

    /// Validate package fields.
    ///
    /// # Arguments
    ///
    /// * `name` - Package display name.
    /// * `binary_name` - Binary name to check for existence.
    /// * `is_custom` - Whether this is a custom package.
    /// * `package_name` - Package name for managed packages.
    /// * `install_command` - Install command for custom packages.
    /// * `manager` - The package manager.
    ///
    /// # Returns
    ///
    /// Validation result.
    pub fn validate_package(
        name: &str,
        binary_name: &str,
        is_custom: bool,
        package_name: &str,
        install_command: &str,
        manager: Option<&PackageManager>,
    ) -> PackageValidation {
        // Validate required fields
        if name.trim().is_empty() {
            warn!("Package validation failed: name is empty");
            return PackageValidation {
                is_valid: false,
                error_message: Some("Name is required".to_string()),
            };
        }

        if binary_name.trim().is_empty() {
            warn!("Package validation failed: binary_name is empty");
            return PackageValidation {
                is_valid: false,
                error_message: Some("Binary name is required".to_string()),
            };
        }

        if manager.is_none() {
            return PackageValidation {
                is_valid: false,
                error_message: Some("Package manager not selected".to_string()),
            };
        }

        // Validate based on package type
        if is_custom {
            // Custom packages require install_command
            if install_command.trim().is_empty() {
                warn!("Package validation failed: install_command is empty for custom package");
                return PackageValidation {
                    is_valid: false,
                    error_message: Some(
                        "Install command is required for custom packages".to_string(),
                    ),
                };
            }
        } else {
            // Managed packages require package_name
            if package_name.trim().is_empty() {
                warn!("Package validation failed: package_name is empty for managed package");
                return PackageValidation {
                    is_valid: false,
                    error_message: Some(
                        "Package name is required for managed packages".to_string(),
                    ),
                };
            }
        }

        PackageValidation {
            is_valid: true,
            error_message: None,
        }
    }

    /// Create a package from validated fields.
    ///
    /// Note: Call `validate_package` first to ensure fields are valid.
    ///
    /// # Arguments
    ///
    /// * Various package fields - see parameters.
    ///
    /// # Returns
    ///
    /// The created package.
    #[must_use]
    pub fn create_package(params: PackageCreationParams) -> Package {
        Package {
            name: params.name.trim().to_string(),
            description: if params.description.trim().is_empty() {
                None
            } else {
                Some(params.description.trim().to_string())
            },
            manager: params.manager,
            package_name: if params.is_custom {
                None
            } else {
                Some(params.package_name.trim().to_string())
            },
            binary_name: params.binary_name.trim().to_string(),
            install_command: if params.is_custom {
                Some(params.install_command.trim().to_string())
            } else {
                None
            },
            existence_check: if params.is_custom && !params.existence_check.trim().is_empty() {
                Some(params.existence_check.trim().to_string())
            } else {
                None
            },
            manager_check: if params.manager_check.trim().is_empty() {
                None
            } else {
                Some(params.manager_check.trim().to_string())
            },
        }
    }

    /// Add a package to a profile in the manifest.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile.
    /// * `package` - The package to add.
    ///
    /// # Returns
    ///
    /// The updated list of packages.
    pub fn add_package(
        repo_path: &Path,
        profile_name: &str,
        package: Package,
    ) -> Result<Vec<Package>> {
        info!(
            "Adding new package: {} (profile: {})",
            package.name, profile_name
        );

        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;

        if let Some(profile) = manifest
            .profiles
            .iter_mut()
            .find(|p| p.name == profile_name)
        {
            profile.packages.push(package);
            let packages = profile.packages.clone();
            manifest.save(repo_path)?;
            Ok(packages)
        } else {
            Err(anyhow::anyhow!("Profile '{profile_name}' not found"))
        }
    }

    /// Update a package in a profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile.
    /// * `index` - Index of the package to update.
    /// * `package` - The updated package.
    ///
    /// # Returns
    ///
    /// The updated list of packages.
    pub fn update_package(
        repo_path: &Path,
        profile_name: &str,
        index: usize,
        package: Package,
    ) -> Result<Vec<Package>> {
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;

        if let Some(profile) = manifest
            .profiles
            .iter_mut()
            .find(|p| p.name == profile_name)
        {
            if index < profile.packages.len() {
                let old_name = profile.packages[index].name.clone();
                info!(
                    "Updating package: {} -> {} (profile: {})",
                    old_name, package.name, profile_name
                );
                profile.packages[index] = package;
                let packages = profile.packages.clone();
                manifest.save(repo_path)?;
                Ok(packages)
            } else {
                Err(anyhow::anyhow!(
                    "Package index {} out of bounds ({} packages)",
                    index,
                    profile.packages.len()
                ))
            }
        } else {
            Err(anyhow::anyhow!("Profile '{profile_name}' not found"))
        }
    }

    /// Delete a package from a profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile.
    /// * `index` - Index of the package to delete.
    ///
    /// # Returns
    ///
    /// The updated list of packages.
    pub fn delete_package(
        repo_path: &Path,
        profile_name: &str,
        index: usize,
    ) -> Result<Vec<Package>> {
        let mut manifest = ProfileManifest::load_or_backfill(repo_path)?;

        if let Some(profile) = manifest
            .profiles
            .iter_mut()
            .find(|p| p.name == profile_name)
        {
            if index < profile.packages.len() {
                let package_name = profile.packages[index].name.clone();
                info!(
                    "Deleting package: {} (index: {}, profile: {})",
                    package_name, index, profile_name
                );
                profile.packages.remove(index);
                let packages = profile.packages.clone();
                manifest.save(repo_path)?;
                Ok(packages)
            } else {
                Err(anyhow::anyhow!(
                    "Package index {} out of bounds ({} packages)",
                    index,
                    profile.packages.len()
                ))
            }
        } else {
            Err(anyhow::anyhow!("Profile '{profile_name}' not found"))
        }
    }

    /// Get packages for a profile.
    ///
    /// # Arguments
    ///
    /// * `repo_path` - Path to the repository.
    /// * `profile_name` - Name of the profile.
    ///
    /// # Returns
    ///
    /// List of packages for the profile.
    pub fn get_packages(repo_path: &Path, profile_name: &str) -> Result<Vec<Package>> {
        let manifest = ProfileManifest::load_or_backfill(repo_path)?;

        Ok(manifest
            .profiles
            .into_iter()
            .find(|p| p.name == profile_name)
            .map(|p| p.packages)
            .unwrap_or_default())
    }

    /// Get the install command builder for a package.
    ///
    /// # Arguments
    ///
    /// * `package` - The package to get the install command for.
    ///
    /// # Returns
    ///
    /// A Command ready to execute.
    #[must_use]
    pub fn get_install_command(package: &Package) -> Command {
        PackageManagerImpl::get_install_command_builder(package)
    }

    /// Start installing a package (async operation).
    ///
    /// Note: This starts an installation process. Use `PackageInstaller::start_install`
    /// for the full async installation flow.
    ///
    /// # Arguments
    ///
    /// * `package` - The package to install.
    ///
    /// # Returns
    ///
    /// Result with installation handle.
    pub fn start_install(
        package: &Package,
    ) -> Result<crate::utils::package_installer::InstallationHandle> {
        PackageInstaller::start_install(package)
    }
}

#[cfg(test)]
mod tests {
    use super::PackageCreationParams;
    use super::*;

    #[test]
    fn test_validate_package_empty_name() {
        let result = PackageService::validate_package(
            "",
            "binary",
            false,
            "package",
            "",
            Some(&PackageManager::Brew),
        );
        assert!(!result.is_valid);
        assert!(result.error_message.unwrap().contains("Name"));
    }

    #[test]
    fn test_validate_package_empty_binary() {
        let result = PackageService::validate_package(
            "name",
            "",
            false,
            "package",
            "",
            Some(&PackageManager::Brew),
        );
        assert!(!result.is_valid);
        assert!(result.error_message.unwrap().contains("Binary"));
    }

    #[test]
    fn test_validate_custom_package_no_install_command() {
        let result = PackageService::validate_package(
            "name",
            "binary",
            true,
            "",
            "",
            Some(&PackageManager::Custom),
        );
        assert!(!result.is_valid);
        assert!(result.error_message.unwrap().contains("Install command"));
    }

    #[test]
    fn test_validate_managed_package_no_package_name() {
        let result = PackageService::validate_package(
            "name",
            "binary",
            false,
            "",
            "",
            Some(&PackageManager::Brew),
        );
        assert!(!result.is_valid);
        assert!(result.error_message.unwrap().contains("Package name"));
    }

    #[test]
    fn test_validate_valid_managed_package() {
        let result = PackageService::validate_package(
            "Git",
            "git",
            false,
            "git",
            "",
            Some(&PackageManager::Brew),
        );
        assert!(result.is_valid);
        assert!(result.error_message.is_none());
    }

    #[test]
    fn test_create_package() {
        let package = PackageService::create_package(PackageCreationParams {
            name: "Git",
            description: "Version control",
            manager: PackageManager::Brew,
            is_custom: false,
            package_name: "git",
            binary_name: "git",
            install_command: "",
            existence_check: "",
            manager_check: "",
        });
        assert_eq!(package.name, "Git");
        assert_eq!(package.description, Some("Version control".to_string()));
        assert_eq!(package.binary_name, "git");
        assert_eq!(package.package_name, Some("git".to_string()));
        assert!(package.install_command.is_none());
    }
}
