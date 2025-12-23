# Package Manager Feature Specification

## Overview

Add a package/dependency manager feature to dotstate that allows users to define CLI tools and dependencies for each profile. The system will check if packages are installed and optionally install missing ones.

## Security & Implementation Notes

### Security Considerations

1. **Shell Injection Prevention**:
   - Managed packages use direct `Command::new()` with args (no shell, no injection risk)
   - Custom packages use `sh -c` (user-provided, user's responsibility)
   - Package names are never shell-escaped for managed packages (direct args prevent injection)

2. **Sudo Handling**:
   - Detect if sudo password is required before attempting installation
   - Show clear error message if password is needed
   - User can configure passwordless sudo or run installation manually

3. **Input Validation**:
   - `package_name` is `Option<String>` (prevents empty string bugs)
   - Custom packages require explicit `install_command` and `existence_check`
   - Managed packages validate that `package_name` is `Some(String)`

### Correctness Improvements

1. **Existence Checks**:
   - Primary: Binary check (`command -v <binary>`)
   - Fallback: Manager-native check (e.g., `brew list <formula>`, `dpkg -s <pkg>`)
   - Handles cases where binary name differs or package installs to non-PATH locations

2. **Output Streaming**:
   - Read stdout and stderr concurrently (prevents deadlocks)
   - Use channels and threads for non-blocking output processing

3. **Non-Blocking Operations**:
   - All checks and installations integrated with event loop
   - State machine pattern for multi-step operations
   - One package per event loop iteration

### UX Improvements

1. **OS-Aware Package Manager List**:
   - Filter managers based on detected OS
   - macOS: brew, cargo, npm, pip, gem
   - Linux: apt/yum/dnf/pacman/snap (based on detection)
   - Reduces noise from irrelevant managers

2. **Visual Feedback**:
   - Status indicators during checks (spinner, green, red, yellow)
   - Live output streaming during installation
   - Clear error messages with actionable guidance

## Data Structure

### TOML Structure

```toml
[[profiles]]
name = "Personal"
description = "My personal Mac setup"
synced_files = [
  ".tmux.conf",
  ".zshrc"
]

  [[profiles.packages]]
  name = "eza"
  manager = "brew"
  package_name = "eza"
  binary_name = "eza"  # Cached, can be derived but stored for performance
  description = "Modern replacement for ls"  # Cached metadata

  [[profiles.packages]]
  name = "bat"
  manager = "brew"
  package_name = "bat"
  binary_name = "bat"  # Cached
  description = "A cat clone with syntax highlighting"  # Cached

  [[profiles.packages]]
  name = "Custom Tool"
  manager = "custom"
  binary_name = "mytool"
  description = "Internal company tool"
  install_command = "./scripts/install-mytool.sh"  # Required for custom
  existence_check = "test -f /usr/local/bin/mytool"  # Required for custom
```

**Note**: For managed packages (non-custom), `install_command` and `existence_check` are derived automatically from the package manager and package_name. Only `package_name`, `manager`, `name`, `binary_name` (cached), and `description` (cached) are stored.

For custom packages, `install_command` and `existence_check` are required and user-provided.

### Rust Data Structures

```rust
// In src/utils/profile_manifest.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileInfo {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub synced_files: Vec<String>,
    #[serde(default)]
    pub packages: Vec<Package>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    /// Display name for the package
    pub name: String,
    /// Optional description (cached metadata, not required)
    #[serde(default)]
    pub description: Option<String>,
    /// Package manager type
    pub manager: PackageManager,
    /// Package name in the manager (e.g., "eza" for brew)
    /// None for custom packages
    #[serde(default)]
    pub package_name: Option<String>,
    /// Binary name to check for existence (cached, can be derived but stored for performance)
    /// For packages with multiple binaries, this is the primary one
    pub binary_name: String,
    /// Optional manager-native check command (fallback when binary_name check fails)
    /// e.g., "brew list eza" or "dpkg -s git"
    #[serde(default)]
    pub manager_check: Option<String>,
    /// Install command (only for custom packages, derived for managed packages)
    #[serde(default)]
    pub install_command: Option<String>,
    /// Command to check if package exists (only for custom packages, derived for managed packages)
    #[serde(default)]
    pub existence_check: Option<String>,
}
```

**Note**:
- For **managed packages**: `install_command` and `existence_check` are `None` - they are derived on-the-fly using `PackageManagerImpl`
- For **custom packages**: `install_command` and `existence_check` are `Some(String)` - user-provided
- `package_name` is `Some(String)` for managed packages, `None` for custom packages (prevents empty string bugs)
- `binary_name` and `description` are cached for performance (avoid re-deriving/fetching)
- `manager_check` is optional fallback for manager-native existence checks (e.g., `brew list <formula>`)

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Brew,      // Homebrew (macOS/Linux)
    Apt,       // Advanced Package Tool (Debian/Ubuntu)
    Yum,       // Yellowdog Updater Modified (RHEL/CentOS)
    Dnf,       // Dandified Yum (Fedora)
    Pacman,    // Arch Linux
    Snap,      // Snap packages
    Cargo,     // Rust packages
    Npm,       // Node.js packages
    Pip,       // Python packages (pip)
    Pip3,      // Python packages (pip3)
    Gem,       // Ruby gems
    Custom,    // Custom install command
}
```

## Package Manager Implementations

### Package Manager Trait/Module

Create `src/utils/package_manager.rs` with implementations for each manager:

```rust
pub struct PackageManagerImpl;

impl PackageManagerImpl {
    /// Build install command as Command struct (no shell injection risk)
    /// For managed packages, we use direct Command::new() instead of sh -c
    pub fn build_install_command(manager: &PackageManager, package_name: &str) -> std::process::Command {
        match manager {
            PackageManager::Brew => {
                let mut cmd = std::process::Command::new("brew");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Apt => {
                let mut cmd = std::process::Command::new("sudo");
                cmd.arg("apt-get").arg("install").arg("-y").arg(package_name);
                cmd
            }
            PackageManager::Yum => {
                let mut cmd = std::process::Command::new("sudo");
                cmd.arg("yum").arg("install").arg("-y").arg(package_name);
                cmd
            }
            PackageManager::Dnf => {
                let mut cmd = std::process::Command::new("sudo");
                cmd.arg("dnf").arg("install").arg("-y").arg(package_name);
                cmd
            }
            PackageManager::Pacman => {
                let mut cmd = std::process::Command::new("sudo");
                cmd.arg("pacman").arg("-S").arg("--noconfirm").arg(package_name);
                cmd
            }
            PackageManager::Snap => {
                let mut cmd = std::process::Command::new("sudo");
                cmd.arg("snap").arg("install").arg(package_name);
                cmd
            }
            PackageManager::Cargo => {
                let mut cmd = std::process::Command::new("cargo");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Npm => {
                let mut cmd = std::process::Command::new("npm");
                cmd.arg("install").arg("-g").arg(package_name);
                cmd
            }
            PackageManager::Pip => {
                let mut cmd = std::process::Command::new("pip");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Pip3 => {
                let mut cmd = std::process::Command::new("pip3");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Gem => {
                let mut cmd = std::process::Command::new("gem");
                cmd.arg("install").arg(package_name);
                cmd
            }
            PackageManager::Custom => {
                // Custom packages use sh -c (user-provided command)
                // This is the only case where we go through shell
                let mut cmd = std::process::Command::new("sh");
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
                std::process::Command::new("sudo")
                    .arg("-n")
                    .arg("true")
                    .output()
                    .map(|o| !o.status.success())
                    .unwrap_or(true) // Assume required if check fails
            }
            _ => false,
        }
    }

    /// Check if binary exists in PATH (no shell, no injection risk)
    /// Implements PATH-walk in Rust for maximum security
    pub fn check_binary_in_path(binary_name: &str) -> bool {
        use std::env;
        use std::path::PathBuf;

        // Get PATH environment variable
        let path_var = env::var("PATH").unwrap_or_default();

        // Split PATH by OS-specific separator
        let path_separator = if cfg!(windows) { ";" } else { ":" };

        for path_dir in path_var.split(path_separator) {
            let mut full_path = PathBuf::from(path_dir);
            full_path.push(binary_name);

            // Check if file exists and is executable
            if full_path.exists() && is_executable(&full_path) {
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

    /// Build manager-native existence check command (fallback)
    /// Used when binary_name check fails or binary_name is missing
    pub fn build_manager_check_command(manager: &PackageManager, package_name: &str) -> Option<std::process::Command> {
        match manager {
            PackageManager::Brew => {
                // Use `brew list <name>` which works for both formulas and casks
                // Note: In v1, we don't distinguish between formulas and casks
                // If user adds a cask, binary check will work, and this fallback will also work
                let mut cmd = std::process::Command::new("brew");
                cmd.arg("list").arg(package_name);
                Some(cmd)
            }
            PackageManager::Apt => {
                let mut cmd = std::process::Command::new("dpkg");
                cmd.arg("-s").arg(package_name);
                Some(cmd)
            }
            PackageManager::Yum | PackageManager::Dnf => {
                let mut cmd = std::process::Command::new("rpm");
                cmd.arg("-q").arg(package_name);
                Some(cmd)
            }
            PackageManager::Pacman => {
                let mut cmd = std::process::Command::new("pacman");
                cmd.arg("-Q").arg(package_name);
                Some(cmd)
            }
            PackageManager::Snap => {
                let mut cmd = std::process::Command::new("snap");
                cmd.arg("list").arg(package_name);
                Some(cmd)
            }
            PackageManager::Cargo => {
                // Cargo doesn't have a native list command, use binary check
                None
            }
            PackageManager::Npm => {
                let mut cmd = std::process::Command::new("npm");
                cmd.arg("list").arg("-g").arg(package_name);
                Some(cmd)
            }
            PackageManager::Pip | PackageManager::Pip3 => {
                let mut cmd = std::process::Command::new("pip");
                if matches!(manager, PackageManager::Pip3) {
                    cmd = std::process::Command::new("pip3");
                }
                cmd.arg("show").arg(package_name);
                Some(cmd)
            }
            PackageManager::Gem => {
                let mut cmd = std::process::Command::new("gem");
                cmd.arg("list").arg("-i").arg(package_name);
                Some(cmd)
            }
            PackageManager::Custom => None, // Custom uses user-provided check
        }
    }

    /// Get install command builder for a package (handles both managed and custom)
    pub fn get_install_command_builder(package: &Package) -> std::process::Command {
        match &package.manager {
            PackageManager::Custom => {
                let command_str = package.install_command.as_ref()
                    .expect("Custom packages must have install_command");
                let mut cmd = std::process::Command::new("sh");
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
        use std::process::Command;

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

    /// Get installation instructions for missing package manager
    pub fn installation_instructions(manager: &PackageManager) -> String {
        match manager {
            PackageManager::Brew => "Install Homebrew: /bin/bash -c \"$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"",
            PackageManager::Apt => "apt-get is usually pre-installed on Debian/Ubuntu systems",
            PackageManager::Yum => "yum is usually pre-installed on RHEL/CentOS systems",
            PackageManager::Dnf => "dnf is usually pre-installed on Fedora systems",
            PackageManager::Pacman => "pacman is usually pre-installed on Arch Linux",
            PackageManager::Snap => "Install snapd: sudo apt-get install snapd (Debian/Ubuntu)",
            PackageManager::Cargo => "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh",
            PackageManager::Npm => "Install Node.js: https://nodejs.org/",
            PackageManager::Pip => "pip usually comes with Python",
            PackageManager::Pip3 => "pip3 usually comes with Python 3",
            PackageManager::Gem => "gem comes with Ruby",
            PackageManager::Custom => "N/A - custom packages don't require a manager",
        }.to_string()
    }
}
```

## UI Components

### New Screen: ManagePackages

Add to `src/ui.rs`:
```rust
pub enum Screen {
    // ... existing screens
    ManagePackages,
}

pub struct PackageManagerState {
    pub list_state: ListState,
    pub packages: Vec<Package>, // From active profile
    pub popup_type: PackagePopupType,
    // Add/Edit popup state
    pub add_name_input: String,
    pub add_name_cursor: usize,
    pub add_description_input: String,
    pub add_description_cursor: usize,
    pub add_manager: Option<PackageManager>,
    pub add_package_name_input: String,
    pub add_package_name_cursor: usize,
    pub add_binary_name_input: String,
    pub add_binary_name_cursor: usize,
    pub add_install_command_input: String, // For custom
    pub add_existence_check_input: String,  // For custom
    pub add_is_advanced: bool,
    pub add_focused_field: AddPackageField,
    // Checking state
    pub is_checking: bool,
    pub checking_index: Option<usize>,
    pub package_statuses: Vec<PackageStatus>, // Installed/NotInstalled/Error
}

pub enum PackagePopupType {
    None,
    Add,
    Edit,
    Delete,
    InstallMissing, // Prompt to install missing packages
}

pub enum AddPackageField {
    Name,
    Description,
    Manager,
    PackageName,      // For managed packages
    BinaryName,
    InstallCommand,   // Custom only
    ExistenceCheck,   // Custom only
    ManagerCheck,     // Optional fallback
}

#[derive(Debug, Clone)]
pub enum PackageStatus {
    Unknown,
    Installed,
    NotInstalled,
    Error(String), // Error message if check failed
}
```

### Component: PackageManagerComponent

Create `src/components/package_manager.rs` similar to `ProfileManagerComponent`:

- Left panel: List of packages with status indicators
- Right panel: Package details (name, description, manager, commands)
- Footer: Context-sensitive help
- Popups: Add, Edit, Delete, Install Missing

### Visual Status Indicators

- ✅ Green: Installed
- ❌ Red: Not installed
- ⚠️ Yellow: Error checking
- 🔄 Spinner: Currently checking

## Installation Flow

### Non-Blocking Installation

Use a state machine similar to GitHub setup:

```rust
pub enum InstallationStep {
    NotStarted,
    Installing {
        package_index: usize,
        package_name: String,
    },
    Complete {
        installed: Vec<usize>,
        failed: Vec<(usize, String)>, // (index, error message)
    },
}
```

### Installation Execution

```rust
// In src/utils/package_installer.rs

pub struct PackageInstaller;

impl PackageInstaller {
    /// Execute install command and capture output (non-blocking, streams output)
    /// For managed packages, uses direct Command (no shell injection)
    /// For custom packages, uses sh -c (user-provided command)
    ///
    /// Reads stdout and stderr concurrently to avoid deadlocks
    ///
    /// Note: on_output must be Arc<dyn Fn(&str) + Send + Sync> because closures
    /// are not clonable. We use Arc for shared ownership across threads.
    pub fn install(
        package: &Package,
        on_output: Arc<dyn Fn(&str) + Send + Sync>, // Callback for live output
    ) -> Result<()> {
        use std::process::{Command, Stdio};
        use std::io::{BufRead, BufReader};
        use std::thread;
        use std::sync::mpsc;
        use std::sync::Arc;

        // Build command (direct Command for managed, sh -c for custom)
        let mut cmd = PackageManagerImpl::get_install_command_builder(package);

        // Check if sudo is required and if password is needed
        if PackageManagerImpl::check_sudo_required(&package.manager) {
            return Err(anyhow::anyhow!(
                "sudo password required. Please run this in a terminal or configure passwordless sudo."
            ));
        }

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Channel for output lines
        let (tx, rx) = mpsc::channel::<String>();
        let on_output_stdout = Arc::clone(&on_output);
        let on_output_stderr = Arc::clone(&on_output);

        // Spawn thread to read stdout
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        let tx_stdout = tx.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx_stdout.send(line);
                }
            }
        });

        // Spawn thread to read stderr
        let stderr = child.stderr.take().ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    let _ = tx.send(format!("[stderr] {}", line));
                }
            }
        });

        // Drop sender in main thread so receiver knows when to stop
        drop(tx);

        // Process output in main thread (non-blocking via channel)
        // This should be called from event loop, not blocking here
        // For now, we'll collect all output, but in real implementation,
        // this should be integrated with the event loop
        for line in rx {
            on_output(&line);
        }

        // Note: on_output_stdout and on_output_stderr are cloned Arc references
        // They are used in the spawned threads above

        let status = child.wait()?;
        if status.success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Installation failed with exit code: {}", status.code().unwrap_or(-1)))
        }
    }

    /// Check if package exists (binary check first, then manager-native fallback)
    /// Returns (exists: bool, used_fallback: bool)
    ///
    /// Important: Binary check is tried FIRST regardless of manager presence.
    /// This allows packages installed manually (without manager) to be detected.
    /// Manager is only required for manager-native fallback and installation.
    pub fn check_exists(package: &Package) -> Result<(bool, bool)> {
        // First, try binary check (no manager required)
        // This works even if package was installed manually
        if PackageManagerImpl::check_binary_in_path(&package.binary_name) {
            return Ok((true, false));
        }

        // Binary check failed, try manager-native check if available
        // This requires the manager to be installed
        if let Some(manager_check) = &package.manager_check {
            // Use custom manager check command (via shell, user-provided)
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(manager_check)
                .output()?;
            return Ok((output.status.success(), true));
        }

        // Try auto-generated manager check (requires manager installed)
        if let Some(package_name) = &package.package_name {
            // Only try manager check if manager is installed
            if PackageManagerImpl::is_manager_installed(&package.manager) {
                if let Some(mut manager_cmd) = PackageManagerImpl::build_manager_check_command(&package.manager, package_name) {
                    let output = manager_cmd.output()?;
                    return Ok((output.status.success(), true));
                }
            }
        }

        // All checks failed
        Ok((false, false))
    }
}
```

### Installation UI

During installation, show:
- Current package being installed
- Live output (if possible) or "Installing..." message
- Progress: "Installing package X of Y"
- Error messages if installation fails
- Continue with next package even if one fails

## Integration Points

### After Profile Activation

In `src/app.rs`, after `activate_profile_after_setup`:

```rust
// After symlinks are created
let manifest = self.load_manifest()?;
if let Some(profile) = manifest.profiles.iter().find(|p| p.name == profile_name) {
    if !profile.packages.is_empty() {
        // Check packages
        self.check_packages(profile.packages.clone())?;
        // Show prompt: "X packages are missing. Install them now?"
        // If yes, start installation flow
    }
}
```

### Package Checking Flow (Non-Blocking)

```rust
// Use a state machine for non-blocking checks (similar to GitHub setup)
pub enum PackageCheckStep {
    NotStarted,
    Checking {
        package_index: usize,
    },
    Complete,
}

impl App {
    fn check_packages_step(&mut self) -> Result<()> {
        let state = &mut self.ui_state.package_manager;

        if state.packages.is_empty() {
            return Ok(());
        }

        // Initialize statuses if needed
        if state.package_statuses.len() != state.packages.len() {
            state.package_statuses = vec![PackageStatus::Unknown; state.packages.len()];
        }

        // Find next unchecked package
        let next_index = state.package_statuses.iter()
            .position(|s| matches!(s, PackageStatus::Unknown));

        if let Some(index) = next_index {
            state.checking_index = Some(index);
            state.is_checking = true;

            let package = &state.packages[index];

            // Check if package manager is installed
            if !PackageManagerImpl::is_manager_installed(&package.manager) {
                state.package_statuses[index] =
                    PackageStatus::Error(format!("Package manager '{}' is not installed", package.manager));
                state.checking_index = None;
                return Ok(()); // Continue on next event loop iteration
            }

            // Check if package exists (binary check + fallback)
            match PackageInstaller::check_exists(package) {
                Ok((true, _)) => {
                    state.package_statuses[index] = PackageStatus::Installed;
                }
                Ok((false, _)) => {
                    state.package_statuses[index] = PackageStatus::NotInstalled;
                }
                Err(e) => {
                    state.package_statuses[index] = PackageStatus::Error(e.to_string());
                }
            }

            state.checking_index = None;

            // Small delay for UX (non-blocking via state machine)
            // In real implementation, use Instant::now() + Duration::from_millis(100)
            // and check in next iteration
        } else {
            // All packages checked
            state.is_checking = false;
        }

        Ok(())
    }
}
```

**Note**: The checking flow is integrated with the event loop. Each call to `check_packages_step()` processes one package, then returns. The event loop calls it repeatedly until all packages are checked. This keeps the UI responsive.

## Error Handling

### Robust Error Handling Strategy

1. **Package Manager Not Installed**: Mark package with error, show instructions
2. **Installation Fails**: Mark package as failed, continue with next
3. **Check Command Fails**: Mark as error, don't block other checks
4. **Invalid Command**: Show error in UI, allow user to edit

### Error Display

- Show error icon (⚠️) next to failed packages
- In details panel, show error message
- Allow user to edit package to fix issues
- Failed packages don't prevent other packages from installing

## User Flow

### Adding a Package

1. User selects "Add New" (A key)
2. Form appears:
   - Name (required)
   - Description (optional, cached for performance)
   - Package Manager (dropdown/list)
   - Package Name (if not custom, required for managed packages)
   - Binary Name (auto-suggested from package_name, editable, cached)
   - [Advanced/Custom mode toggle] → Shows Install Command and Existence Check fields (required for custom)
3. On save:
   - Validate fields:
     - Managed packages: require `package_name`, `binary_name` (auto-suggested)
     - Custom packages: require `install_command`, `existence_check`
   - For managed packages: commands are NOT stored, only derived on-the-fly
   - For custom packages: `install_command` and `existence_check` are stored
   - Save to manifest (only source data + cached fields)
   - Immediately check if installed
   - If not installed, prompt: "Package 'X' is not installed. Install now?"

### Checking Packages

1. User presses "C" (Check)
2. UI shows spinner next to each package as it's checked (non-blocking, via event loop)
3. Check process:
   - First tries binary check (`command -v <binary>`)
   - If that fails, tries manager-native check (e.g., `brew list <formula>`)
   - Falls back to custom `manager_check` if provided
4. Status updates: ✅ (green), ❌ (red), ⚠️ (yellow for errors)
5. After check completes, if any are missing:
   - Show popup: "X packages are missing. Install them now? (Y/N)"
   - If Yes: Start installation flow (non-blocking)
   - If No: Return to package list

### Installing Packages

1. User confirms installation
2. Check sudo requirements:
   - If sudo password needed, show error: "sudo password required. Please run in a terminal or configure passwordless sudo."
   - User can configure passwordless sudo or run installation manually
3. For each missing package (in order, non-blocking via event loop):
   - Show "Installing X of Y: package_name"
   - Execute install command:
     - **Managed packages**: Direct `Command::new()` (no shell, no injection risk)
     - **Custom packages**: `sh -c` (user-provided command)
   - Stream stdout and stderr **concurrently** (avoid deadlocks)
   - Show live output in UI
   - On success: Mark as installed, continue
   - On failure: Mark as failed, show error, continue (robust error handling)
4. After all installations:
   - Show summary: "Installed: X, Failed: Y"
   - List failed packages with errors
   - User can edit failed packages and retry

## Implementation Phases

### Phase 1: Data Structure & Basic UI
- [ ] Add `Package` and `PackageManager` types
- [ ] Update `ProfileInfo` to include `packages` field
- [ ] Update manifest loading/saving
- [ ] Create `PackageManagerComponent` skeleton
- [ ] Add "Manage Packages" menu item
- [ ] Basic list view (no status checking yet)

### Phase 2: Package Manager Support
- [ ] Implement `PackageManagerImpl` with all managers
- [ ] Command generation for each manager
- [ ] Existence check generation
- [ ] Binary name suggestion
- [ ] Package manager detection

### Phase 3: Package Checking
- [ ] Implement `PackageInstaller::check_exists()` - binary check FIRST (no manager required), then fallback
- [ ] Binary check uses PATH-walk (no shell, no injection)
- [ ] Manager-native check only if manager is installed
- [ ] Add checking state machine (non-blocking, event loop integrated)
- [ ] Visual status indicators
- [ ] Non-blocking check execution (one package per event loop iteration)
- [ ] Error handling for checks
- [ ] Order: binary check first, then manager-native fallback

### Phase 4: Add/Edit UI
- [ ] Add package form (standard mode)
- [ ] Package manager selection (OS-filtered list)
- [ ] Auto-suggest binary_name from package_name
- [ ] Show derived commands in details panel (read-only, for managed packages)
- [ ] Custom mode toggle
- [ ] Custom mode: show install_command and existence_check fields (required)
- [ ] Optional manager_check field (for fallback checks)
- [ ] Edit existing package
- [ ] Delete package confirmation
- [ ] Validation:
  - Managed packages: require `package_name` (Some(String))
  - Custom packages: require `install_command` and `existence_check`, `package_name` is None

### Phase 5: Installation Flow
- [ ] Installation state machine (non-blocking, event loop integrated)
- [ ] Non-blocking installation execution
- [ ] **Concurrent stdout/stderr reading** (avoid deadlocks)
- [ ] Live output streaming (use Arc<dyn Fn(&str) + Send + Sync> for callbacks)
- [ ] Progress display
- [ ] Sudo password detection and warning
- [ ] Error handling and recovery (continue on failure)
- [ ] Installation summary
- [ ] **Do NOT auto-install package managers** - only show instructions

### Phase 6: Integration
- [ ] Package check after profile activation
- [ ] Package check after profile switch
- [ ] Prompt for missing packages
- [ ] Save packages to manifest

### Phase 7: Polish
- [ ] Keyboard shortcuts
- [ ] Mouse support
- [ ] Error messages
- [ ] Help text
- [ ] Unit tests

## Open Questions Resolved

✅ **Package Manager Support**: Support all listed managers, offer to install missing ones
✅ **Installation Flow**: Run commands, show live output if possible, install one by one
✅ **Ordering**: Respect entry order, no UI for reordering
✅ **Platform**: No platform-specific actions
✅ **Validation**: Robust error handling, mark failures, no strict validation
✅ **Status Tracking**: Manual check only, no version tracking
✅ **UI/UX**: Live output preferred, visual status indicators, non-blocking
✅ **Integration**: Check after activation, ask before installing, never auto-install
✅ **Advanced**: User provides install and check commands
✅ **Data Structure**: Packages array under each profile, order by entry

## Notes

- Installation is always optional and user-confirmed
- Failed packages don't block other packages
- **Package managers are NOT automatically installed** - we only provide installation instructions
- Package manager must be installed before packages can be **installed** (not required for checking)
- **Check order matters**: Binary check is tried FIRST (no manager required), then manager-native fallback
- This allows manually installed packages to be detected even if manager is missing
- Order matters: packages are installed in the order they appear in the array
- Custom packages give full control to the user
- All package data is stored in the manifest (version controlled)
- **Data Storage Strategy**:
  - Managed packages: Only store source data (`package_name: Option<String>`, `manager`, `name`, `binary_name`, `description`)
  - Commands (`install_command`, `existence_check`) are derived on-the-fly, not stored
  - Custom packages: Store `install_command` and `existence_check` (user-provided), `package_name` is None
  - `binary_name` and `description` are cached for performance (avoid re-deriving/fetching)
- **Security**:
  - Managed packages: Use direct `Command::new()` with args (no shell, no injection risk)
  - Custom packages: Use `sh -c` (user-provided, user's responsibility)
  - Package names are never shell-escaped for managed packages (direct args)
  - Binary checks use PATH-walk in Rust (no shell, no `command -v` via Command::new())
  - Manager presence checks use PATH-walk (reuses binary check logic)
- **Correctness**:
  - Existence checks: Try binary check FIRST (no manager required), fallback to manager-native check
  - Binary check uses PATH-walk in Rust (no shell, no injection risk)
  - Manager-native checks handle cases where binary name differs or package installs to non-PATH
  - Manager is only required for manager-native fallback and installation, not for binary check
  - Order matters: binary check first allows manually installed packages to be detected
  - Concurrent stdout/stderr reading prevents deadlocks
  - Brew: Uses `brew list <name>` (works for both formulas and casks, no --formula restriction)
  - Pip/Pip3: May have false negatives due to venv/system python differences (acceptable per spec)
- **UX**:
  - Package manager list is OS-filtered (only shows relevant managers)
  - Sudo password detection warns user before attempting installation
  - All operations are non-blocking (integrated with event loop)
  - Visual feedback during checks and installations

