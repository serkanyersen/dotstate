use crate::utils::package_manager::PackageManagerImpl;
use crate::utils::profile_manifest::Package;
use anyhow::Result;
use std::io::{BufRead, BufReader};
use std::process::{Child, Stdio};
use std::sync::mpsc;
use std::thread;
use tracing::debug;

/// Package installer and checker utilities
pub struct PackageInstaller;

/// Installation process handle for non-blocking operations
#[allow(dead_code)] // Reserved for future use
pub struct InstallationHandle {
    pub child: Child,
    pub output_rx: mpsc::Receiver<String>,
}

use crate::ui::InstallationStatus;

impl PackageInstaller {
    /// Synchronous install that streams status to a sender
    pub fn install(package: &Package, tx: mpsc::Sender<InstallationStatus>) {
        match Self::start_install(package) {
            Ok(handle) => {
                let InstallationHandle {
                    mut child,
                    output_rx,
                } = handle;
                // Stream output
                for line in output_rx {
                    let _ = tx.send(InstallationStatus::Output(line));
                }
                // Wait for process to finish
                match child.wait() {
                    Ok(status) => {
                        let success = status.success();
                        let error = if success {
                            None
                        } else {
                            Some("Installation failed".to_string())
                        };
                        let _ = tx.send(InstallationStatus::Complete { success, error });
                    }
                    Err(e) => {
                        let _ = tx.send(InstallationStatus::Complete {
                            success: false,
                            error: Some(e.to_string()),
                        });
                    }
                }
            }
            Err(e) => {
                let _ = tx.send(InstallationStatus::Complete {
                    success: false,
                    error: Some(e.to_string()),
                });
            }
        }
    }

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

        let mut child = cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;

        // Channel for output lines
        let (tx, rx) = mpsc::channel::<String>();
        let tx_stdout = tx.clone();
        let tx_stderr = tx.clone();

        // Spawn thread to read stdout
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stdout"))?;
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            #[allow(clippy::unnecessary_lazy_evaluations, clippy::lines_filter_map_ok)]
            for line in reader.lines().flatten() {
                let _ = tx_stdout.send(line);
            }
        });

        // Spawn thread to read stderr
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to capture stderr"))?;
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            #[allow(clippy::unnecessary_lazy_evaluations, clippy::lines_filter_map_ok)]
            for line in reader.lines().flatten() {
                let _ = tx_stderr.send(format!("[stderr] {}", line));
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
    /// Returns (exists: bool, check_command: Option<String>, output: Option<String>)
    ///
    /// Important: Binary check is tried FIRST regardless of manager presence.
    /// This allows packages installed manually (without manager) to be detected.
    /// Manager is only required for manager-native fallback and installation.
    pub fn check_exists(package: &Package) -> Result<(bool, Option<String>, Option<String>)> {
        // First, try binary check (no manager required)
        // This works even if package was installed manually
        debug!(
            "Checking if binary '{}' exists in PATH for package {}",
            package.binary_name, package.name
        );
        if PackageManagerImpl::check_binary_in_path(&package.binary_name) {
            debug!("Package {} found via binary check", package.name);
            return Ok((
                true,
                Some(format!("which {}", package.binary_name)),
                Some("Binary found in PATH".to_string()),
            ));
        }
        debug!("Binary '{}' not found in PATH", package.binary_name);

        // Binary check failed, try manager-native check if available
        // This requires the manager to be installed
        if let Some(manager_check) = &package.manager_check {
            debug!("Trying custom manager check for package {}", package.name);
            // Use custom manager check command (via shell, user-provided)
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg(manager_check)
                .output()?;
            let found = output.status.success();
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let combined_output = format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);

            debug!("Custom manager check for {}: {}", package.name, found);
            return Ok((found, Some(manager_check.clone()), Some(combined_output)));
        }

        // Try auto-generated manager check (requires manager installed)
        if let Some(package_name) = &package.package_name {
            // Only try manager check if manager is installed
            if PackageManagerImpl::is_manager_installed(&package.manager) {
                debug!(
                    "Trying auto-generated manager check for package {} (manager: {:?})",
                    package.name, package.manager
                );
                if let Some(mut manager_cmd) =
                    PackageManagerImpl::build_manager_check_command(&package.manager, package_name)
                {
                    // Capture command string representation for cache
                    let cmd_str = format!("{:?}", manager_cmd);

                    let output = manager_cmd.output()?;
                    let found = output.status.success();
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    let combined_output = format!("STDOUT:\n{}\nSTDERR:\n{}", stdout, stderr);

                    debug!(
                        "Auto-generated manager check for {}: {}",
                        package.name, found
                    );
                    return Ok((found, Some(cmd_str), Some(combined_output)));
                }
            } else {
                debug!(
                    "Manager {:?} not installed, skipping manager check for {}",
                    package.manager, package.name
                );
            }
        }

        // All checks failed
        debug!("All checks failed for package {}", package.name);
        Ok((
            false,
            None,
            Some("No suitable check method found".to_string()),
        ))
    }
}
