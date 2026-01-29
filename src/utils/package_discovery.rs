//! Package discovery module for detecting installed system packages.
//!
//! This module provides functionality to discover packages installed via
//! various package managers, focusing on explicitly installed packages
//! (not dependencies).

use anyhow::{Context, Result};
use std::process::Command;
use std::sync::mpsc;
use std::thread;
use tracing::{info, warn};

/// A discovered package from a system package manager.
#[derive(Debug, Clone)]
pub struct DiscoveredPackage {
    /// The package name as known by the package manager
    pub package_name: String,
    /// The binary name (if detectable)
    pub binary_name: Option<String>,
    /// Description (if available)
    pub description: Option<String>,
    /// The package manager this was discovered from
    pub manager: DiscoverySource,
}

/// Source package manager for discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscoverySource {
    Homebrew,
    Pacman,
    Apt,
    Dnf,
    Yum,
    Snap,
    Cargo,
    Npm,
    Pip,
    Pip3,
    Gem,
}

impl DiscoverySource {
    /// Get display name for the source.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            DiscoverySource::Homebrew => "Homebrew",
            DiscoverySource::Pacman => "Pacman",
            DiscoverySource::Apt => "APT",
            DiscoverySource::Dnf => "DNF",
            DiscoverySource::Yum => "YUM",
            DiscoverySource::Snap => "Snap",
            DiscoverySource::Cargo => "Cargo",
            DiscoverySource::Npm => "NPM",
            DiscoverySource::Pip => "pip",
            DiscoverySource::Pip3 => "pip3",
            DiscoverySource::Gem => "Gem",
        }
    }

    /// Convert to the profile manifest `PackageManager` type.
    #[must_use]
    pub fn to_package_manager(&self) -> crate::utils::profile_manifest::PackageManager {
        use crate::utils::profile_manifest::PackageManager;
        match self {
            DiscoverySource::Homebrew => PackageManager::Brew,
            DiscoverySource::Pacman => PackageManager::Pacman,
            DiscoverySource::Apt => PackageManager::Apt,
            DiscoverySource::Dnf => PackageManager::Dnf,
            DiscoverySource::Yum => PackageManager::Yum,
            DiscoverySource::Snap => PackageManager::Snap,
            DiscoverySource::Cargo => PackageManager::Cargo,
            DiscoverySource::Npm => PackageManager::Npm,
            DiscoverySource::Pip => PackageManager::Pip,
            DiscoverySource::Pip3 => PackageManager::Pip3,
            DiscoverySource::Gem => PackageManager::Gem,
        }
    }

    /// Try to convert from a `PackageManager`.
    /// Returns None for Custom (which doesn't have packages to discover).
    #[must_use]
    pub fn from_package_manager(
        manager: &crate::utils::profile_manifest::PackageManager,
    ) -> Option<Self> {
        use crate::utils::profile_manifest::PackageManager;
        match manager {
            PackageManager::Brew => Some(DiscoverySource::Homebrew),
            PackageManager::Pacman => Some(DiscoverySource::Pacman),
            PackageManager::Apt => Some(DiscoverySource::Apt),
            PackageManager::Dnf => Some(DiscoverySource::Dnf),
            PackageManager::Yum => Some(DiscoverySource::Yum),
            PackageManager::Snap => Some(DiscoverySource::Snap),
            PackageManager::Cargo => Some(DiscoverySource::Cargo),
            PackageManager::Npm => Some(DiscoverySource::Npm),
            PackageManager::Pip => Some(DiscoverySource::Pip),
            PackageManager::Pip3 => Some(DiscoverySource::Pip3),
            PackageManager::Gem => Some(DiscoverySource::Gem),
            PackageManager::Custom => None, // Custom doesn't support discovery
        }
    }

    /// Check if this source supports package discovery.
    /// Some managers can list installed packages, others cannot.
    #[must_use]
    pub fn supports_discovery(&self) -> bool {
        match self {
            DiscoverySource::Homebrew
            | DiscoverySource::Pacman
            | DiscoverySource::Apt
            | DiscoverySource::Dnf
            | DiscoverySource::Yum
            | DiscoverySource::Snap
            | DiscoverySource::Cargo
            | DiscoverySource::Npm
            | DiscoverySource::Pip
            | DiscoverySource::Pip3
            | DiscoverySource::Gem => true,
        }
    }
}

/// Status updates from async discovery
#[derive(Debug, Clone)]
pub enum DiscoveryStatus {
    /// Discovery started for a source
    Started(DiscoverySource),
    /// Discovery completed successfully
    Complete {
        source: DiscoverySource,
        packages: Vec<DiscoveredPackage>,
    },
    /// Discovery failed
    Failed {
        source: DiscoverySource,
        error: String,
    },
    /// No package managers available
    NoSourcesAvailable,
}

/// Trait for package manager discovery implementations.
pub trait PackageDiscoverer: Send + Sync {
    /// Check if this package manager is available on the system.
    fn is_available(&self) -> bool;

    /// Get the discovery source type.
    fn source(&self) -> DiscoverySource;

    /// Discover explicitly installed packages.
    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>>;

    /// Try to detect the binary name for a package.
    fn detect_binary_name(&self, package_name: &str) -> Option<String>;
}

/// Homebrew package discoverer.
pub struct HomebrewDiscoverer;

impl PackageDiscoverer for HomebrewDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("brew")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Homebrew
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering Homebrew packages...");

        // Use `brew leaves` to get only explicitly installed packages (not dependencies)
        let output = Command::new("brew")
            .args(["leaves", "--installed-on-request"])
            .output()
            .context("Failed to run brew leaves")?;

        if !output.status.success() {
            // Fallback to just `brew leaves` if --installed-on-request isn't supported
            let output = Command::new("brew")
                .arg("leaves")
                .output()
                .context("Failed to run brew leaves")?;

            if !output.status.success() {
                anyhow::bail!("brew leaves failed");
            }

            return self.parse_leaves_output(&output.stdout);
        }

        self.parse_leaves_output(&output.stdout)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        // Try to get the binary name from brew
        // First, check if a binary with the same name exists
        let check = Command::new("which").arg(package_name).output().ok()?;

        if check.status.success() {
            return Some(package_name.to_string());
        }

        // Try to list files installed by the package and find executables
        let output = Command::new("brew")
            .args(["list", "--formula", package_name])
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            // Look for files in bin/ directories
            for line in stdout.lines() {
                if line.contains("/bin/") {
                    if let Some(binary) = line.split('/').next_back() {
                        if !binary.is_empty() {
                            return Some(binary.to_string());
                        }
                    }
                }
            }
        }

        // Default to package name
        Some(package_name.to_string())
    }
}

impl HomebrewDiscoverer {
    fn parse_leaves_output(&self, stdout: &[u8]) -> Result<Vec<DiscoveredPackage>> {
        let stdout = String::from_utf8_lossy(stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let package_name = line.trim();
            if package_name.is_empty() {
                continue;
            }

            // Use package name as binary name by default (fast)
            // Skip the slow detect_binary_name call - user can edit if needed
            packages.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                binary_name: Some(package_name.to_string()),
                description: None,
                manager: DiscoverySource::Homebrew,
            });
        }

        info!("Discovered {} Homebrew packages", packages.len());
        Ok(packages)
    }
}

/// Pacman package discoverer (Arch Linux).
pub struct PacmanDiscoverer;

impl PackageDiscoverer for PacmanDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("pacman")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Pacman
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering Pacman packages...");

        // Use `pacman -Qe` to get explicitly installed packages
        let output = Command::new("pacman")
            .args(["-Qe", "-q"]) // -q for quiet (just names)
            .output()
            .context("Failed to run pacman -Qe")?;

        if !output.status.success() {
            anyhow::bail!("pacman -Qe failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let package_name = line.trim();
            if package_name.is_empty() {
                continue;
            }

            // Use package name as binary name by default (fast)
            packages.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                binary_name: Some(package_name.to_string()),
                description: None,
                manager: DiscoverySource::Pacman,
            });
        }

        info!("Discovered {} Pacman packages", packages.len());
        Ok(packages)
    }

    #[allow(dead_code)]
    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        // Check if binary with same name exists
        let check = Command::new("which").arg(package_name).output().ok()?;

        if check.status.success() {
            return Some(package_name.to_string());
        }

        // Try pacman -Ql to list files
        let output = Command::new("pacman")
            .args(["-Ql", package_name])
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("/usr/bin/") || line.contains("/bin/") {
                    if let Some(binary) = line.split_whitespace().last() {
                        if let Some(name) = binary.split('/').next_back() {
                            if !name.is_empty() {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        Some(package_name.to_string())
    }
}

/// APT package discoverer (Debian/Ubuntu).
pub struct AptDiscoverer;

impl PackageDiscoverer for AptDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("apt-mark")
            .arg("--help")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Apt
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering APT packages...");

        // Use `apt-mark showmanual` to get manually installed packages
        let output = Command::new("apt-mark")
            .arg("showmanual")
            .output()
            .context("Failed to run apt-mark showmanual")?;

        if !output.status.success() {
            anyhow::bail!("apt-mark showmanual failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            let package_name = line.trim();
            if package_name.is_empty() {
                continue;
            }

            // Use package name as binary name by default (fast)
            packages.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                binary_name: Some(package_name.to_string()),
                description: None,
                manager: DiscoverySource::Apt,
            });
        }

        info!("Discovered {} APT packages", packages.len());
        Ok(packages)
    }

    #[allow(dead_code)]
    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        // Check if binary with same name exists
        let check = Command::new("which").arg(package_name).output().ok()?;

        if check.status.success() {
            return Some(package_name.to_string());
        }

        // Try dpkg -L to list files
        let output = Command::new("dpkg")
            .args(["-L", package_name])
            .output()
            .ok()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("/usr/bin/") || line.contains("/bin/") {
                    if let Some(name) = line.split('/').next_back() {
                        if !name.is_empty() {
                            return Some(name.to_string());
                        }
                    }
                }
            }
        }

        Some(package_name.to_string())
    }
}

/// YUM package discoverer (RHEL/CentOS).
pub struct YumDiscoverer;

impl PackageDiscoverer for YumDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("yum")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Yum
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering YUM packages...");

        let output = Command::new("yum")
            .args(["list", "installed"])
            .output()
            .context("Failed to run yum list installed")?;

        if !output.status.success() {
            anyhow::bail!("yum list installed failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines().skip(1) {
            // Skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let package_full = parts[0];
            // Remove .arch suffix (e.g., "git.x86_64" -> "git")
            let package_name = package_full.split('.').next().unwrap_or(package_full);

            packages.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                binary_name: Some(package_name.to_string()),
                description: None,
                manager: DiscoverySource::Yum,
            });
        }

        info!("Discovered {} YUM packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// DNF package discoverer (Fedora).
pub struct DnfDiscoverer;

impl PackageDiscoverer for DnfDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("dnf")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Dnf
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering DNF packages...");

        let output = Command::new("dnf")
            .args(["list", "installed"])
            .output()
            .context("Failed to run dnf list installed")?;

        if !output.status.success() {
            anyhow::bail!("dnf list installed failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines().skip(1) {
            // Skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let package_full = parts[0];
            // Remove .arch suffix
            let package_name = package_full.split('.').next().unwrap_or(package_full);

            packages.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                binary_name: Some(package_name.to_string()),
                description: None,
                manager: DiscoverySource::Dnf,
            });
        }

        info!("Discovered {} DNF packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// Snap package discoverer.
pub struct SnapDiscoverer;

impl PackageDiscoverer for SnapDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("snap")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Snap
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering Snap packages...");

        let output = Command::new("snap")
            .arg("list")
            .output()
            .context("Failed to run snap list")?;

        if !output.status.success() {
            anyhow::bail!("snap list failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines().skip(1) {
            // Skip header
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let package_name = parts[0];

            packages.push(DiscoveredPackage {
                package_name: package_name.to_string(),
                binary_name: Some(package_name.to_string()),
                description: None,
                manager: DiscoverySource::Snap,
            });
        }

        info!("Discovered {} Snap packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// Cargo package discoverer (Rust).
pub struct CargoDiscoverer;

impl PackageDiscoverer for CargoDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("cargo")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Cargo
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering Cargo packages...");

        let output = Command::new("cargo")
            .args(["install", "--list"])
            .output()
            .context("Failed to run cargo install --list")?;

        if !output.status.success() {
            anyhow::bail!("cargo install --list failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            // Lines with package names don't start with whitespace
            if !line.starts_with(' ') && !line.is_empty() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(package_name) = parts.first() {
                    packages.push(DiscoveredPackage {
                        package_name: package_name.to_string(),
                        binary_name: Some(package_name.to_string()),
                        description: None,
                        manager: DiscoverySource::Cargo,
                    });
                }
            }
        }

        info!("Discovered {} Cargo packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// NPM package discoverer.
pub struct NpmDiscoverer;

impl PackageDiscoverer for NpmDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("npm")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Npm
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering NPM packages...");

        let output = Command::new("npm")
            .args(["list", "-g", "--depth=0", "--parseable"])
            .output()
            .context("Failed to run npm list -g")?;

        if !output.status.success() {
            // npm list returns non-zero even on success sometimes
            warn!("npm list returned non-zero status, continuing anyway");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            // Lines are paths, e.g. /usr/lib/node_modules/npm or /usr/lib/node_modules/@scope/pkg
            // We want to extract the package name part after .../node_modules/
            if let Some(pos) = line.rfind("/node_modules/") {
                let package_part = &line[pos + 14..];
                if !package_part.is_empty() {
                    let package_name = package_part.to_string();
                    packages.push(DiscoveredPackage {
                        package_name: package_name.clone(),
                        binary_name: Some(package_name), // Usually binary name matches package name
                        description: None,
                        manager: DiscoverySource::Npm,
                    });
                }
            } else if let Some(package_name) = line.split('/').next_back() {
                // Fallback for flat paths if node_modules not found (unlikely with -g)
                if !package_name.is_empty()
                    && package_name != "lib"
                    && package_name != "node_modules"
                {
                    packages.push(DiscoveredPackage {
                        package_name: package_name.to_string(),
                        binary_name: Some(package_name.to_string()),
                        description: None,
                        manager: DiscoverySource::Npm,
                    });
                }
            }
        }

        info!("Discovered {} NPM packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// Pip package discoverer (Python 2/3).
pub struct PipDiscoverer;

impl PackageDiscoverer for PipDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("pip")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Pip
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering pip packages...");

        let output = Command::new("pip")
            .args(["list", "--format=freeze"])
            .output()
            .context("Failed to run pip list")?;

        if !output.status.success() {
            anyhow::bail!("pip list failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            if let Some(package_name) = line.split("==").next() {
                if !package_name.is_empty() {
                    packages.push(DiscoveredPackage {
                        package_name: package_name.to_string(),
                        binary_name: Some(package_name.to_string()),
                        description: None,
                        manager: DiscoverySource::Pip,
                    });
                }
            }
        }

        info!("Discovered {} pip packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// Pip3 package discoverer (Python 3).
pub struct Pip3Discoverer;

impl PackageDiscoverer for Pip3Discoverer {
    fn is_available(&self) -> bool {
        Command::new("pip3")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Pip3
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering pip3 packages...");

        let output = Command::new("pip3")
            .args(["list", "--format=freeze"])
            .output()
            .context("Failed to run pip3 list")?;

        if !output.status.success() {
            anyhow::bail!("pip3 list failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            if let Some(package_name) = line.split("==").next() {
                if !package_name.is_empty() {
                    packages.push(DiscoveredPackage {
                        package_name: package_name.to_string(),
                        binary_name: Some(package_name.to_string()),
                        description: None,
                        manager: DiscoverySource::Pip3,
                    });
                }
            }
        }

        info!("Discovered {} pip3 packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// Gem package discoverer (Ruby).
pub struct GemDiscoverer;

impl PackageDiscoverer for GemDiscoverer {
    fn is_available(&self) -> bool {
        Command::new("gem")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn source(&self) -> DiscoverySource {
        DiscoverySource::Gem
    }

    fn discover_packages(&self) -> Result<Vec<DiscoveredPackage>> {
        info!("Discovering gem packages...");

        let output = Command::new("gem")
            .arg("list")
            .output()
            .context("Failed to run gem list")?;

        if !output.status.success() {
            anyhow::bail!("gem list failed");
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::new();

        for line in stdout.lines() {
            if let Some(package_name) = line.split_whitespace().next() {
                if !package_name.is_empty() && !line.starts_with("***") {
                    packages.push(DiscoveredPackage {
                        package_name: package_name.to_string(),
                        binary_name: Some(package_name.to_string()),
                        description: None,
                        manager: DiscoverySource::Gem,
                    });
                }
            }
        }

        info!("Discovered {} gem packages", packages.len());
        Ok(packages)
    }

    fn detect_binary_name(&self, package_name: &str) -> Option<String> {
        Some(package_name.to_string())
    }
}

/// Main discovery service that aggregates all discoverers.
pub struct PackageDiscoveryService {
    discoverers: Vec<Box<dyn PackageDiscoverer>>,
}

impl Default for PackageDiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageDiscoveryService {
    #[must_use]
    pub fn new() -> Self {
        let discoverers: Vec<Box<dyn PackageDiscoverer>> = vec![
            Box::new(HomebrewDiscoverer),
            Box::new(PacmanDiscoverer),
            Box::new(AptDiscoverer),
            Box::new(YumDiscoverer),
            Box::new(DnfDiscoverer),
            Box::new(SnapDiscoverer),
            Box::new(CargoDiscoverer),
            Box::new(NpmDiscoverer),
            Box::new(PipDiscoverer),
            Box::new(Pip3Discoverer),
            Box::new(GemDiscoverer),
        ];

        Self { discoverers }
    }

    /// Get available package managers on this system.
    #[must_use]
    pub fn available_sources(&self) -> Vec<DiscoverySource> {
        self.discoverers
            .iter()
            .filter(|d| d.is_available())
            .map(|d| d.source())
            .collect()
    }

    /// Discover packages from a specific source.
    /// Returns empty list for sources that don't have discovery implemented yet.
    pub fn discover_from(&self, source: DiscoverySource) -> Result<Vec<DiscoveredPackage>> {
        // Check if this source supports discovery
        if !source.supports_discovery() {
            return Ok(Vec::new()); // Unsupported sources return empty list
        }

        for discoverer in &self.discoverers {
            if discoverer.source() == source {
                return discoverer.discover_packages();
            }
        }
        // No discoverer found - return empty
        Ok(Vec::new())
    }

    /// Discover packages from all available sources.
    pub fn discover_all(&self) -> Vec<DiscoveredPackage> {
        let mut all_packages = Vec::new();

        for discoverer in &self.discoverers {
            if discoverer.is_available() {
                match discoverer.discover_packages() {
                    Ok(packages) => all_packages.extend(packages),
                    Err(e) => {
                        warn!(
                            "Failed to discover packages from {:?}: {}",
                            discoverer.source(),
                            e
                        );
                    }
                }
            }
        }

        all_packages
    }

    /// Start async discovery - returns a receiver for status updates.
    /// The discovery runs in a background thread and sends status updates.
    #[must_use]
    pub fn discover_async() -> mpsc::Receiver<DiscoveryStatus> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let service = PackageDiscoveryService::new();
            let sources = service.available_sources();

            if sources.is_empty() {
                let _ = tx.send(DiscoveryStatus::NoSourcesAvailable);
                return;
            }

            // Use the first available source
            let source = sources[0];
            let _ = tx.send(DiscoveryStatus::Started(source));

            match service.discover_from(source) {
                Ok(packages) => {
                    let _ = tx.send(DiscoveryStatus::Complete { source, packages });
                }
                Err(e) => {
                    let _ = tx.send(DiscoveryStatus::Failed {
                        source,
                        error: e.to_string(),
                    });
                }
            }
        });

        rx
    }

    /// Start async discovery for a specific source.
    /// Returns a receiver for status updates. Discovery runs in a background thread.
    #[must_use]
    pub fn discover_source_async(source: DiscoverySource) -> mpsc::Receiver<DiscoveryStatus> {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let service = PackageDiscoveryService::new();
            let _ = tx.send(DiscoveryStatus::Started(source));

            match service.discover_from(source) {
                Ok(packages) => {
                    let _ = tx.send(DiscoveryStatus::Complete { source, packages });
                }
                Err(e) => {
                    let _ = tx.send(DiscoveryStatus::Failed {
                        source,
                        error: e.to_string(),
                    });
                }
            }
        });

        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_source_display_name() {
        assert_eq!(DiscoverySource::Homebrew.display_name(), "Homebrew");
        assert_eq!(DiscoverySource::Pacman.display_name(), "Pacman");
        assert_eq!(DiscoverySource::Apt.display_name(), "APT");
    }

    #[test]
    fn test_service_creation() {
        let service = PackageDiscoveryService::new();
        // Just verify it doesn't panic
        let _ = service.available_sources();
    }
}
