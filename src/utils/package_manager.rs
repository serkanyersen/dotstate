use std::path::PathBuf;
use std::process::Command;
use crate::utils::profile_manifest::{Package, PackageManager};

/// Package manager implementation utilities
pub struct PackageManagerImpl;

impl PackageManagerImpl {
    /// Check if binary exists in PATH (no shell, no injection risk)
    /// Implements PATH-walk in Rust for maximum security
    pub fn check_binary_in_path(binary_name: &str) -> bool {
        use std::env;

        // Get PATH environment variable
        let path_var = env::var("PATH").unwrap_or_default();

        // Split PATH by OS-specific separator
        let path_separator = if cfg!(windows) { ";" } else { ":" };

        for path_dir in path_var.split(path_separator) {
            let mut full_path = PathBuf::from(path_dir);
            full_path.push(binary_name);

            // Check if file exists and is executable
            if full_path.exists() && Self::is_executable(&full_path) {
                return true;
            }
        }

        false
    }

    /// Check if a file is executable (cross-platform)
    fn is_executable(path: &std::path::Path) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(path) {
                let perms = metadata.permissions();
                return perms.mode() & 0o111 != 0; // Check execute bit
            }
        }

        #[cfg(windows)]
        {
            // On Windows, .exe/.bat/.cmd files are considered executable
            if let Some(ext) = path.extension() {
                return matches!(ext.to_str(), Some("exe") | Some("bat") | Some("cmd") | Some("com"));
            }
        }

        false
    }

    /// Check if package manager is installed
    /// Uses PATH-walk (no shell) for consistency and security
    pub fn is_manager_installed(manager: &PackageManager) -> bool {
        let binary_name = match manager {
            PackageManager::Brew => "brew",
            PackageManager::Apt => "apt-get",
            PackageManager::Yum => "yum",
            PackageManager::Dnf => "dnf",
            PackageManager::Pacman => "pacman",
            PackageManager::Snap => "snap",
            PackageManager::Cargo => "cargo",
            PackageManager::Npm => "npm",
            PackageManager::Pip => "pip",
            PackageManager::Pip3 => "pip3",
            PackageManager::Gem => "gem",
            PackageManager::Custom => return true, // Always available
        };

        Self::check_binary_in_path(binary_name)
    }

    /// Build install command as Command struct (no shell injection risk)
    /// For managed packages, we use direct Command::new() instead of sh -c
    pub fn build_install_command(manager: &PackageManager, package_name: &str) -> Command {
        match manager {
            PackageManager::Brew => {
                let mut cmd = Command::new("brew");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Apt => {
                let mut cmd = Command::new("sudo");
                cmd.arg("apt-get").arg("install").arg("-y").arg(package_name);
                cmd
            }
            PackageManager::Yum => {
                let mut cmd = Command::new("sudo");
                cmd.arg("yum").arg("install").arg("-y").arg(package_name);
                cmd
            }
            PackageManager::Dnf => {
                let mut cmd = Command::new("sudo");
                cmd.arg("dnf").arg("install").arg("-y").arg(package_name);
                cmd
            }
            PackageManager::Pacman => {
                let mut cmd = Command::new("sudo");
                cmd.arg("pacman").arg("-S").arg("--noconfirm").arg(package_name);
                cmd
            }
            PackageManager::Snap => {
                let mut cmd = Command::new("sudo");
                cmd.arg("snap").arg("install").arg(package_name);
                cmd
            }
            PackageManager::Cargo => {
                let mut cmd = Command::new("cargo");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Npm => {
                let mut cmd = Command::new("npm");
                cmd.arg("install").arg("-g").arg(package_name);
                cmd
            }
            PackageManager::Pip => {
                let mut cmd = Command::new("pip");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Pip3 => {
                let mut cmd = Command::new("pip3");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Gem => {
                let mut cmd = Command::new("gem");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Custom => {
                // Custom packages use sh -c (user-provided command)
                // This is the only case where we go through shell
                let mut cmd = Command::new("sh");
                cmd.arg("-c");
                // Command will be set by caller
                cmd
            }
        }
    }

    /// Check if sudo password is required (for sudo-based installs)
    pub fn check_sudo_required(manager: &PackageManager) -> bool {
        match manager {
            PackageManager::Apt | PackageManager::Yum | PackageManager::Dnf
            | PackageManager::Pacman | PackageManager::Snap => {
                // Check if sudo -n (non-interactive) succeeds
                Command::new("sudo")
                    .arg("-n")
                    .arg("true")
                    .output()
                    .map(|o| !o.status.success())
                    .unwrap_or(true) // Assume required if check fails
            }
            _ => false,
        }
    }

    /// Build manager-native existence check command (fallback)
    /// Used when binary_name check fails or binary_name is missing
    pub fn build_manager_check_command(manager: &PackageManager, package_name: &str) -> Option<Command> {
        match manager {
            PackageManager::Brew => {
                // Use `brew list <name>` which works for both formulas and casks
                // Note: In v1, we don't distinguish between formulas and casks in the UI
                // If user adds a cask (e.g., via custom package or by using cask name),
                // binary check will work, and this fallback will also work
                // For formulas, this works. For casks, this also works.
                let mut cmd = Command::new("brew");
                cmd.arg("list").arg(package_name);
                Some(cmd)
            }
            PackageManager::Apt => {
                let mut cmd = Command::new("dpkg");
                cmd.arg("-s").arg(package_name);
                Some(cmd)
            }
            PackageManager::Yum | PackageManager::Dnf => {
                let mut cmd = Command::new("rpm");
                cmd.arg("-q").arg(package_name);
                Some(cmd)
            }
            PackageManager::Pacman => {
                let mut cmd = Command::new("pacman");
                cmd.arg("-Q").arg(package_name);
                Some(cmd)
            }
            PackageManager::Snap => {
                let mut cmd = Command::new("snap");
                cmd.arg("list").arg(package_name);
                Some(cmd)
            }
            PackageManager::Cargo => {
                // Cargo doesn't have a native list command, use binary check
                None
            }
            PackageManager::Npm => {
                let mut cmd = Command::new("npm");
                cmd.arg("list").arg("-g").arg(package_name);
                Some(cmd)
            }
            PackageManager::Pip | PackageManager::Pip3 => {
                let mut cmd = Command::new("pip");
                if matches!(manager, PackageManager::Pip3) {
                    cmd = Command::new("pip3");
                }
                cmd.arg("show").arg(package_name);
                Some(cmd)
            }
            PackageManager::Gem => {
                let mut cmd = Command::new("gem");
                cmd.arg("list").arg("-i").arg(package_name);
                Some(cmd)
            }
            PackageManager::Custom => None, // Custom uses user-provided check
        }
    }

    /// Get install command builder for a package (handles both managed and custom)
    pub fn get_install_command_builder(package: &Package) -> Command {
        match &package.manager {
            PackageManager::Custom => {
                let command_str = package.install_command.as_ref()
                    .expect("Custom packages must have install_command");
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(command_str);
                cmd
            }
            _ => {
                let package_name = package.package_name.as_ref()
                    .expect("Managed packages must have package_name");
                Self::build_install_command(&package.manager, package_name)
            }
        }
    }

    /// Get available package managers for current OS
    /// Filters out managers that are unlikely to be installed on this system
    pub fn get_available_managers() -> Vec<PackageManager> {
        let mut available = Vec::new();

        // Detect OS
        let os = std::env::consts::OS;

        // Always available (OS-specific)
        match os {
            "macos" => {
                // macOS: brew is common, others are possible
                if Self::is_manager_installed(&PackageManager::Brew) {
                    available.push(PackageManager::Brew);
                }
            }
            "linux" => {
                // Linux: detect which package manager is available
                if Self::is_manager_installed(&PackageManager::Apt) {
                    available.push(PackageManager::Apt);
                }
                if Self::is_manager_installed(&PackageManager::Yum) {
                    available.push(PackageManager::Yum);
                }
                if Self::is_manager_installed(&PackageManager::Dnf) {
                    available.push(PackageManager::Dnf);
                }
                if Self::is_manager_installed(&PackageManager::Pacman) {
                    available.push(PackageManager::Pacman);
                }
                if Self::is_manager_installed(&PackageManager::Snap) {
                    available.push(PackageManager::Snap);
                }
            }
            _ => {}
        }

        // Language package managers (cross-platform, check if installed)
        if Self::is_manager_installed(&PackageManager::Cargo) {
            available.push(PackageManager::Cargo);
        }
        if Self::is_manager_installed(&PackageManager::Npm) {
            available.push(PackageManager::Npm);
        }
        if Self::is_manager_installed(&PackageManager::Pip) {
            available.push(PackageManager::Pip);
        }
        if Self::is_manager_installed(&PackageManager::Pip3) {
            available.push(PackageManager::Pip3);
        }
        if Self::is_manager_installed(&PackageManager::Gem) {
            available.push(PackageManager::Gem);
        }

        // Custom is always available
        available.push(PackageManager::Custom);

        available
    }

    /// Suggest binary name from package name
    pub fn suggest_binary_name(package_name: &str) -> String {
        // Most package managers use the same name
        // Some exceptions: brew install git -> binary is "git"
        package_name.to_string()
    }

    /// Get installation instructions for missing package manager
    /// Note: We do NOT automatically install package managers.
    /// We only provide instructions for the user to install manually.
    pub fn installation_instructions(manager: &PackageManager) -> String {
        match manager {
            PackageManager::Brew => "Install Homebrew: /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"".to_string(),
            PackageManager::Apt => "apt-get is usually pre-installed on Debian/Ubuntu systems".to_string(),
            PackageManager::Yum => "yum is usually pre-installed on RHEL/CentOS systems".to_string(),
            PackageManager::Dnf => "dnf is usually pre-installed on Fedora systems".to_string(),
            PackageManager::Pacman => "pacman is usually pre-installed on Arch Linux".to_string(),
            PackageManager::Snap => "Install snapd: sudo apt-get install snapd (Debian/Ubuntu)".to_string(),
            PackageManager::Cargo => "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh".to_string(),
            PackageManager::Npm => "Install Node.js: https://nodejs.org/".to_string(),
            PackageManager::Pip => "pip usually comes with Python".to_string(),
            PackageManager::Pip3 => "pip3 usually comes with Python 3".to_string(),
            PackageManager::Gem => "gem comes with Ruby".to_string(),
            PackageManager::Custom => "N/A - custom packages don't require a manager".to_string(),
        }
    }
}

