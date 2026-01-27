//! Upgrade command for checking and installing updates.

use crate::version_check::{check_for_updates_now, current_version, UpdateInfo};
use anyhow::{Context, Result};
use std::io::{self, Write};

/// Execute the upgrade command.
pub fn execute(check_only: bool) -> Result<()> {
    println!("🔍 Checking for updates...");
    println!("   Current version: {}", current_version());
    println!();

    match check_for_updates_now() {
        Some(update_info) => {
            println!(
                "🎉 New version available: {} (current: {})",
                update_info.latest_version, update_info.current_version
            );
            println!();
            println!("📝 Release notes: {}", update_info.release_url);
            println!();

            if check_only {
                // Just show update options without prompting
                println!("Update options:");
                println!();
                println!("  1. Using install script:");
                println!(
                    "     curl -fsSL {} | bash",
                    UpdateInfo::install_script_url()
                );
                println!();
                println!("  2. Using Cargo:");
                println!("     cargo install dotstate --force");
                println!();
                println!("  3. Using Homebrew:");
                println!("     brew upgrade dotstate");
                println!();
                println!("Run 'dotstate upgrade' (without --check) for interactive upgrade.");
                return Ok(());
            }

            // Interactive mode
            println!("How would you like to update?");
            println!();
            println!("  1. Run install script (recommended)");
            println!("     Downloads and installs the latest binary.");
            println!("     ⚠️  Warning: May conflict with cargo/brew installations.");
            println!();
            println!("  2. Show manual update commands");
            println!("     Display commands for cargo, brew, or manual download.");
            println!();
            println!("  3. Cancel");
            println!();
            print!("Enter choice [1-3]: ");

            io::stdout().flush().context("Failed to flush stdout")?;

            let mut input = String::new();
            io::stdin()
                .read_line(&mut input)
                .context("Failed to read input")?;

            let choice = input.trim();

            match choice {
                "1" => {
                    println!();
                    println!("⚠️  This will download and run the install script from:");
                    println!("   {}", UpdateInfo::install_script_url());
                    println!();
                    println!("   This may conflict with cargo or homebrew installations.");
                    println!(
                        "   If you installed via cargo or brew, consider using those to update."
                    );
                    println!();
                    print!("Continue? [y/N]: ");
                    io::stdout().flush().context("Failed to flush stdout")?;

                    let mut confirm = String::new();
                    io::stdin()
                        .read_line(&mut confirm)
                        .context("Failed to read input")?;

                    let confirmed = confirm.trim().to_lowercase();
                    if confirmed != "y" && confirmed != "yes" {
                        println!("Cancelled.");
                        return Ok(());
                    }

                    println!();
                    println!("📥 Running install script...");
                    println!();

                    // Run the install script
                    let status = std::process::Command::new("bash")
                        .arg("-c")
                        .arg(format!(
                            "curl -fsSL {} | bash",
                            UpdateInfo::install_script_url()
                        ))
                        .status()
                        .context("Failed to run install script")?;

                    if status.success() {
                        println!();
                        println!(
                            "✅ Update complete! Please restart dotstate to use the new version."
                        );
                    } else {
                        eprintln!();
                        eprintln!(
                            "❌ Install script failed with exit code: {}",
                            status.code().unwrap_or(-1)
                        );
                        eprintln!("   Try updating manually using one of the other methods.");
                        std::process::exit(1);
                    }
                }
                "2" => {
                    println!();
                    println!("Manual update options:");
                    println!();
                    println!("  Using install script:");
                    println!("    curl -fsSL {} | bash", UpdateInfo::install_script_url());
                    println!();
                    println!("  Using Cargo:");
                    println!("    cargo install dotstate --force");
                    println!();
                    println!("  Using Homebrew:");
                    println!("    brew upgrade dotstate");
                    println!();
                    println!("  Direct download:");
                    println!("    {}", UpdateInfo::releases_url());
                }
                "3" | "" => {
                    println!("Cancelled.");
                }
                _ => {
                    println!("Invalid choice. Cancelled.");
                }
            }
        }
        None => {
            println!(
                "✅ You're running the latest version ({})!",
                current_version()
            );
        }
    }

    Ok(())
}
