use anyhow::Result;
use std::sync::mpsc;
use std::process::{Stdio, Child};
use std::io::{BufRead, BufReader};
use std::thread;
use crate::utils::profile_manifest::Package;
use crate::utils::package_manager::PackageManagerImpl;

/// Package installer and checker utilities
pub struct PackageInstaller;

/// Installation process handle for non-blocking operations
#[allow(dead_code)] // Reserved for future use
pub struct InstallationHandle {
    pub child: Child,
    pub output_rx: mpsc::Receiver<String>,
}

impl PackageInstaller {
    /// Start installation process (non-blocking)
    /// Returns a handle that can be used to check progress and read output
    /// The caller is responsible for checking if the process is done and reading output
    #[allow(dead_code)] // Reserved for future use
    pub fn start_install(package: &Package) -> Result<InstallationHandle> {
        // Check if sudo is required and if password is needed
        if PackageManagerImpl::check_sudo_required(&package.manager) {
            return Err(anyhow::anyhow!(
                "sudo password required. Please run this in a terminal or configure passwordless sudo."
            ));
        }

        // Build command (direct Command for managed, sh -c for custom)
        let mut cmd = PackageManagerImpl::get_install_command_builder(package);

        let mut child = cmd
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Channel for output lines
        let (tx, rx) = mpsc::channel::<String>();
        let tx_stdout = tx.clone();
        let tx_stderr = tx.clone();

        // Spawn thread to read stdout
        let stdout = child.stdout.take().ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
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
                    let _ = tx_stderr.send(format!("[stderr] {}", line));
                }
            }
        });

        Ok(InstallationHandle {
            child,
            output_rx: rx,
        })
    }

    /// Check if installation process is complete
    #[allow(dead_code)] // Reserved for future use
    pub fn check_installation_status(handle: &mut InstallationHandle) -> Result<Option<bool>> {
        // Try to wait for the process (non-blocking)
        match handle.child.try_wait()? {
            Some(status) => Ok(Some(status.success())),
            None => Ok(None), // Still running
        }
    }

    /// Read available output lines (non-blocking)
    #[allow(dead_code)] // Reserved for future use
    pub fn read_output(handle: &InstallationHandle) -> Vec<String> {
        let mut lines = Vec::new();
        // Try to read all available lines without blocking
        while let Ok(line) = handle.output_rx.try_recv() {
            lines.push(line);
        }
        lines
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

