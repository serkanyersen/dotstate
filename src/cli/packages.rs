//! Package management CLI commands.
//!
//! This module provides CLI commands for managing packages in profiles:
//! - `list` - List packages for a profile
//! - `add` - Add a package to a profile
//! - `remove` - Remove a package from a profile
//! - `check` - Check installation status of packages
//! - `install` - Install all missing packages
//! - `help` - Show help for packages commands

use crate::cli::common::{
    parse_manager, print_error, print_success, print_warning, prompt_confirm, prompt_manager,
    prompt_select_with_suffix, prompt_string, prompt_string_optional, CliContext,
};
use crate::services::{PackageCheckStatus, PackageCreationParams, PackageService};
use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum PackagesCommand {
    /// List packages for a profile
    List {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Show detailed package information
        #[arg(short, long)]
        verbose: bool,
    },
    /// Add a package to a profile
    Add {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Package display name
        #[arg(short, long)]
        name: Option<String>,
        /// Package manager (brew, cargo, apt, npm, pip, custom, etc.)
        #[arg(short, long)]
        manager: Option<String>,
        /// Binary name to check for existence
        #[arg(short, long)]
        binary: Option<String>,
        /// Optional description
        #[arg(long)]
        description: Option<String>,
        /// Package name in the manager (defaults to binary name)
        #[arg(long)]
        package_name: Option<String>,
        /// Install command (required for custom manager)
        #[arg(long)]
        install_command: Option<String>,
        /// Command to check if package exists (optional, for custom)
        #[arg(long)]
        existence_check: Option<String>,
    },
    /// Remove a package from a profile
    Remove {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
        /// Package name to remove
        name: Option<String>,
    },
    /// Check installation status of packages
    Check {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
    },
    /// Install all missing packages
    Install {
        /// Target profile (defaults to active profile)
        #[arg(short, long)]
        profile: Option<String>,
        /// Show package manager output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Show help for packages commands
    Help {
        /// Command to show help for
        command: Option<String>,
    },
}

/// Execute a packages subcommand.
pub fn execute(command: PackagesCommand) -> Result<()> {
    match command {
        PackagesCommand::List { profile, verbose } => cmd_list(profile, verbose),
        PackagesCommand::Add {
            profile,
            name,
            manager,
            binary,
            description,
            package_name,
            install_command,
            existence_check,
        } => cmd_add(
            profile,
            name,
            manager,
            binary,
            description,
            package_name,
            install_command,
            existence_check,
        ),
        PackagesCommand::Remove { profile, yes, name } => cmd_remove(profile, yes, name),
        PackagesCommand::Check { profile } => cmd_check(profile),
        PackagesCommand::Install { profile, verbose } => cmd_install(profile, verbose),
        PackagesCommand::Help { command } => cmd_help(command),
    }
}

fn cmd_help(command: Option<String>) -> Result<()> {
    match command.as_deref() {
        Some("list") => {
            println!("Usage: dotstate packages list [OPTIONS]");
            println!();
            println!("List packages for a profile");
            println!();
            println!("Options:");
            println!("  -p, --profile <NAME>  Target profile (defaults to active profile)");
            println!("  -v, --verbose         Show detailed package information");
        }
        Some("add") => {
            println!("Usage: dotstate packages add [OPTIONS]");
            println!();
            println!("Add a package to a profile");
            println!();
            println!("Options:");
            println!("  -p, --profile <NAME>       Target profile (defaults to active profile)");
            println!("  -n, --name <NAME>          Package display name");
            println!("  -m, --manager <MANAGER>    Package manager (brew, cargo, apt, npm, pip, custom, etc.)");
            println!("  -b, --binary <NAME>        Binary name to check for existence");
            println!("      --description <TEXT>   Optional description");
            println!("      --package-name <NAME>  Package name in the manager (defaults to binary name)");
            println!(
                "      --install-command <CMD>  Install command (required for custom manager)"
            );
            println!(
                "      --existence-check <CMD>  Command to check if package exists (optional)"
            );
            println!();
            println!("Examples:");
            println!("  dotstate packages add -n ripgrep -m brew -b rg");
            println!("  dotstate packages add --profile Work -n neovim -m apt -b nvim");
            println!("  dotstate packages add  # Interactive mode");
        }
        Some("remove") => {
            println!("Usage: dotstate packages remove [OPTIONS] [NAME]");
            println!();
            println!("Remove a package from a profile");
            println!();
            println!("Options:");
            println!("  -p, --profile <NAME>  Target profile (defaults to active profile)");
            println!("  -y, --yes             Skip confirmation prompt");
            println!();
            println!("Examples:");
            println!("  dotstate packages remove ripgrep");
            println!("  dotstate packages remove --profile Work neovim");
            println!("  dotstate packages remove  # Interactive selection");
        }
        Some("check") => {
            println!("Usage: dotstate packages check [OPTIONS]");
            println!();
            println!("Check installation status of packages");
            println!();
            println!("Options:");
            println!("  -p, --profile <NAME>  Target profile (defaults to active profile)");
        }
        Some("install") => {
            println!("Usage: dotstate packages install [OPTIONS]");
            println!();
            println!("Install all missing packages for a profile");
            println!();
            println!("Options:");
            println!("  -p, --profile <NAME>  Target profile (defaults to active profile)");
            println!("  -v, --verbose         Show package manager output");
        }
        Some(cmd) => {
            eprintln!("Unknown command: {cmd}");
            eprintln!("Available commands: list, add, remove, check, install");
            std::process::exit(1);
        }
        None => {
            println!("dotstate packages - Manage packages for profiles");
            println!();
            println!("Usage: dotstate packages <COMMAND>");
            println!();
            println!("Commands:");
            println!("  list     List packages for a profile");
            println!("  add      Add a package to a profile");
            println!("  remove   Remove a package from a profile");
            println!("  check    Check installation status of packages");
            println!("  install  Install all missing packages");
            println!("  help     Show help for a command");
            println!();
            println!("Options:");
            println!("  -h, --help  Print help");
            println!();
            println!("Run 'dotstate packages help <command>' for more info on a command.");
        }
    }
    Ok(())
}

fn cmd_list(profile: Option<String>, verbose: bool) -> Result<()> {
    use crate::utils::package_cache::PackageCache;

    let ctx = CliContext::load()?;
    let profile_name = ctx.resolve_profile(profile.as_deref());

    // Validate profile exists
    if !ctx.profile_exists(&profile_name) {
        print_error(&format!("Profile '{profile_name}' not found"));
        std::process::exit(1);
    }

    let is_active = ctx.is_active_profile(&profile_name);
    let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

    // Initialize cache for updating status
    let mut cache = PackageCache::new().unwrap_or_default();

    if packages.is_empty() {
        println!("No packages configured for profile '{profile_name}'");
        println!("Use 'dotstate packages add' to add packages.");
        return Ok(());
    }

    println!("Packages for profile '{profile_name}':\n");

    let mut installed_count = 0;
    let mut missing_count = 0;

    for package in &packages {
        let manager_str = format!("{:?}", package.manager).to_lowercase();

        if is_active {
            // Check installation status for active profile
            let check_result = PackageService::check_package(package);
            let (installed, status_str) = match check_result.status {
                PackageCheckStatus::Installed => {
                    installed_count += 1;
                    (Some(true), "\u{2713} installed".to_string())
                }
                PackageCheckStatus::NotInstalled => {
                    missing_count += 1;
                    (Some(false), "\u{2717} not installed".to_string())
                }
                PackageCheckStatus::Error(ref e) => (None, format!("? {e}")),
                PackageCheckStatus::Unknown => (None, "? unknown".to_string()),
            };

            // Update cache with check result
            if let Some(is_installed) = installed {
                let _ = cache.update_status(
                    &profile_name,
                    &package.name,
                    is_installed,
                    None, // check_command not exposed by PackageService
                    None, // output not exposed by PackageService
                );
            }

            if verbose {
                println!("  {}", package.name);
                println!("    Manager: {manager_str}");
                if let Some(ref pkg_name) = package.package_name {
                    println!("    Package: {pkg_name}");
                }
                println!("    Binary: {}", package.binary_name);
                if let Some(ref desc) = package.description {
                    println!("    Description: {desc}");
                }
                if let Some(ref cmd) = package.install_command {
                    println!("    Install: {cmd}");
                }
                if let Some(ref check) = package.existence_check {
                    println!("    Check: {check}");
                }
                println!("    Status: {status_str}");
                println!();
            } else {
                println!("  {:<12} {:<8} {}", package.name, manager_str, status_str);
            }
        } else {
            // Non-active profile - no status checks
            if verbose {
                println!("  {}", package.name);
                println!("    Manager: {manager_str}");
                if let Some(ref pkg_name) = package.package_name {
                    println!("    Package: {pkg_name}");
                }
                println!("    Binary: {}", package.binary_name);
                if let Some(ref desc) = package.description {
                    println!("    Description: {desc}");
                }
                if let Some(ref cmd) = package.install_command {
                    println!("    Install: {cmd}");
                }
                if let Some(ref check) = package.existence_check {
                    println!("    Check: {check}");
                }
                println!();
            } else {
                println!("  {:<12} {}", package.name, manager_str);
            }
        }
    }

    // Summary
    if is_active {
        println!(
            "\n{} packages ({} installed, {} missing)",
            packages.len(),
            installed_count,
            missing_count
        );
    } else {
        println!("\n{} packages", packages.len());
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn cmd_add(
    profile: Option<String>,
    name: Option<String>,
    manager: Option<String>,
    binary: Option<String>,
    description: Option<String>,
    package_name: Option<String>,
    install_command: Option<String>,
    existence_check: Option<String>,
) -> Result<()> {
    let ctx = CliContext::load()?;
    let profile_name = ctx.resolve_profile(profile.as_deref());

    // Validate profile exists
    if !ctx.profile_exists(&profile_name) {
        print_error(&format!("Profile '{profile_name}' not found"));
        std::process::exit(1);
    }

    let is_active = ctx.is_active_profile(&profile_name);

    // Get existing packages to check for duplicates
    let existing = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

    // Prompt for missing required fields
    let name = match name {
        Some(n) => n,
        None => prompt_string("Name", None)?,
    };

    // Check for duplicate
    if existing.iter().any(|p| p.name == name) {
        print_error(&format!(
            "Package '{name}' already exists in profile '{profile_name}'"
        ));
        std::process::exit(1);
    }

    let manager = match manager {
        Some(m) => parse_manager(&m).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid manager '{m}'. Valid: brew, apt, cargo, npm, pip, custom, etc."
            )
        })?,
        None => prompt_manager(is_active)?,
    };

    let is_custom = matches!(
        manager,
        crate::utils::profile_manifest::PackageManager::Custom
    );

    let binary = match binary {
        Some(b) => b,
        None => prompt_string("Binary name", None)?,
    };

    // For non-custom, get package_name (defaults to binary)
    let pkg_name = if is_custom {
        String::new()
    } else {
        match package_name {
            Some(p) => p,
            None => prompt_string("Package name in manager", Some(&binary))?,
        }
    };

    // For custom, get install_command (required)
    let install_cmd = if is_custom {
        match install_command {
            Some(c) => c,
            None => prompt_string("Install command", None)?,
        }
    } else {
        String::new()
    };

    // For custom, get existence_check (optional)
    let exist_check = if is_custom {
        match existence_check {
            Some(c) => Some(c),
            None => prompt_string_optional("Existence check")?,
        }
    } else {
        None
    };

    // Description is always optional
    let desc = match description {
        Some(d) => d,
        None => prompt_string_optional("Description")?.unwrap_or_default(),
    };

    // Validate
    let validation = PackageService::validate_package(
        &name,
        &binary,
        is_custom,
        &pkg_name,
        &install_cmd,
        Some(&manager),
    );

    if !validation.is_valid {
        print_error(
            &validation
                .error_message
                .unwrap_or_else(|| "Validation failed".to_string()),
        );
        std::process::exit(1);
    }

    // Create package
    let package = PackageService::create_package(PackageCreationParams {
        name: &name,
        description: &desc,
        manager: manager.clone(),
        is_custom,
        package_name: &pkg_name,
        binary_name: &binary,
        install_command: &install_cmd,
        existence_check: exist_check.as_deref().unwrap_or(""),
        manager_check: "",
    });

    // Add to profile
    PackageService::add_package(&ctx.config.repo_path, &profile_name, package)?;

    print_success(&format!(
        "Package '{name}' added to profile '{profile_name}'"
    ));

    Ok(())
}

fn cmd_remove(profile: Option<String>, yes: bool, name: Option<String>) -> Result<()> {
    let ctx = CliContext::load()?;
    let profile_name = ctx.resolve_profile(profile.as_deref());

    // Validate profile exists
    if !ctx.profile_exists(&profile_name) {
        print_error(&format!("Profile '{profile_name}' not found"));
        std::process::exit(1);
    }

    let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

    if packages.is_empty() {
        println!("No packages found in profile '{profile_name}'");
        return Ok(());
    }

    // Find package by name or prompt for selection
    let (index, package_name, manager_str) = if let Some(ref n) = name {
        // Find by name
        if let Some(i) = packages.iter().position(|p| p.name == *n) {
            let mgr = format!("{:?}", packages[i].manager).to_lowercase();
            (i, n.clone(), mgr)
        } else {
            print_error(&format!(
                "Package '{n}' not found in profile '{profile_name}'"
            ));
            std::process::exit(1);
        }
    } else {
        // Interactive selection
        println!("Select package to remove from profile '{profile_name}':\n");
        let options: Vec<(String, Option<String>)> = packages
            .iter()
            .map(|p| {
                let mgr = format!("{:?}", p.manager).to_lowercase();
                let suffix = format!("({mgr})");
                (p.name.clone(), Some(suffix))
            })
            .collect();

        let options_ref: Vec<(&str, Option<&str>)> = options
            .iter()
            .map(|(n, s)| (n.as_str(), s.as_deref()))
            .collect();

        let selected = prompt_select_with_suffix("Package", &options_ref)?;
        let mgr = format!("{:?}", packages[selected].manager).to_lowercase();
        (selected, packages[selected].name.clone(), mgr)
    };

    // Confirm unless --yes
    if !yes {
        let confirm_msg =
            format!("Remove '{package_name}' ({manager_str}) from profile '{profile_name}'?");
        if !prompt_confirm(&confirm_msg)? {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Delete package
    PackageService::delete_package(&ctx.config.repo_path, &profile_name, index)?;

    print_success(&format!(
        "Package '{package_name}' removed from profile '{profile_name}'"
    ));

    Ok(())
}

fn cmd_check(profile: Option<String>) -> Result<()> {
    use crate::utils::package_cache::PackageCache;

    let ctx = CliContext::load()?;
    let profile_name = ctx.resolve_profile(profile.as_deref());

    // Validate profile exists
    if !ctx.profile_exists(&profile_name) {
        print_error(&format!("Profile '{profile_name}' not found"));
        std::process::exit(1);
    }

    // Check can only work for active profile
    if !ctx.is_active_profile(&profile_name) {
        print_warning(&format!(
            "Cannot check installation status for non-active profile '{profile_name}'"
        ));
        println!("   Packages may be for a different system. Use 'list' to view packages.");
        return Ok(());
    }

    let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

    if packages.is_empty() {
        println!("No packages configured for profile '{profile_name}'");
        return Ok(());
    }

    // Initialize cache for updating status
    let mut cache = PackageCache::new().unwrap_or_default();

    println!("Checking packages for profile '{profile_name}'...\n");

    let mut installed = 0;
    let mut not_installed = 0;
    let mut errors = 0;

    for package in &packages {
        let manager_str = format!("{:?}", package.manager).to_lowercase();
        let result = PackageService::check_package(package);

        let (is_installed, status_str) = match result.status {
            PackageCheckStatus::Installed => {
                installed += 1;
                (Some(true), "\u{2713} installed")
            }
            PackageCheckStatus::NotInstalled => {
                not_installed += 1;
                (Some(false), "\u{2717} not installed")
            }
            PackageCheckStatus::Error(ref e) => {
                errors += 1;
                // Print inline for errors
                println!("  {:<12} {:<8} ? {}", package.name, manager_str, e);
                continue;
            }
            PackageCheckStatus::Unknown => {
                errors += 1;
                (None, "? unknown")
            }
        };

        // Update cache with check result
        if let Some(installed_status) = is_installed {
            let _ = cache.update_status(
                &profile_name,
                &package.name,
                installed_status,
                None, // check_command not exposed by PackageService
                None, // output not exposed by PackageService
            );
        }

        println!("  {:<12} {:<8} {}", package.name, manager_str, status_str);
    }

    println!();
    if not_installed > 0 {
        println!(
            "{} of {} packages installed ({} missing)",
            installed,
            packages.len(),
            not_installed
        );
    } else {
        println!("{} of {} packages installed", installed, packages.len());
    }

    if errors > 0 {
        println!("({errors} check errors)");
    }

    Ok(())
}

fn cmd_install(profile: Option<String>, verbose: bool) -> Result<()> {
    use crate::utils::package_installer::PackageInstaller;
    use std::sync::mpsc;
    use std::thread;

    let ctx = CliContext::load()?;
    let profile_name = ctx.resolve_profile(profile.as_deref());

    // Validate profile exists
    if !ctx.profile_exists(&profile_name) {
        print_error(&format!("Profile '{profile_name}' not found"));
        std::process::exit(1);
    }

    // Install can only work for active profile
    if !ctx.is_active_profile(&profile_name) {
        print_warning(&format!(
            "Cannot install packages for non-active profile '{profile_name}'"
        ));
        println!("   Switch to this profile first or install manually on the target system.");
        return Ok(());
    }

    let packages = PackageService::get_packages(&ctx.config.repo_path, &profile_name)?;

    if packages.is_empty() {
        println!("No packages configured for profile '{profile_name}'");
        return Ok(());
    }

    // Find missing packages
    let missing: Vec<_> = packages
        .iter()
        .filter(|p| {
            let result = PackageService::check_package(p);
            matches!(result.status, PackageCheckStatus::NotInstalled)
        })
        .collect();

    if missing.is_empty() {
        print_success("All packages are already installed");
        return Ok(());
    }

    println!(
        "Installing {} missing package(s) for profile '{}'...\n",
        missing.len(),
        profile_name
    );

    let mut success_count = 0;
    let mut fail_count = 0;

    for package in missing {
        let manager_str = format!("{:?}", package.manager).to_lowercase();

        if verbose {
            println!("Installing {} ({})...", package.name, manager_str);
        }

        // Spawn install in background thread so we can stream output in real-time
        let (tx, rx) = mpsc::channel();
        let pkg_clone = package.clone();
        thread::spawn(move || {
            PackageInstaller::install(&pkg_clone, tx);
        });

        // Stream output as it arrives
        let mut install_success = false;
        let mut error_msg = None;

        for status in rx {
            match status {
                crate::ui::InstallationStatus::Output(line) => {
                    if verbose {
                        println!("{line}");
                    }
                }
                crate::ui::InstallationStatus::Complete { success, error } => {
                    install_success = success;
                    error_msg = error;
                }
            }
        }

        if install_success {
            success_count += 1;
            println!("  \u{2713} {} ({})", package.name, manager_str);
        } else {
            fail_count += 1;
            let err = error_msg.unwrap_or_else(|| "Unknown error".to_string());
            println!("  \u{2717} {} ({}) - {}", package.name, manager_str, err);
        }
    }

    println!();
    if fail_count == 0 {
        print_success(&format!("{success_count} package(s) installed"));
    } else {
        println!(
            "{} of {} package(s) installed ({} failed)",
            success_count,
            success_count + fail_count,
            fail_count
        );
    }

    Ok(())
}
