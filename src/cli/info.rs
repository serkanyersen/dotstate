//! Info commands: help, logs, config, repository.

use anyhow::{Context, Result};
use clap::CommandFactory;

use super::Cli;

/// Execute the help command.
pub fn cmd_help(command: Option<String>) -> Result<()> {
    if let Some(cmd_name) = command {
        // Show help for a specific command
        let mut cli = Cli::command();
        if let Some(subcommand) = cli.find_subcommand_mut(&cmd_name) {
            let help = subcommand.render_help();
            println!("{help}");
        } else {
            eprintln!("❌ Unknown command: {cmd_name}");
            eprintln!("\nAvailable commands:");
            print_all_commands();
            std::process::exit(1);
        }
    } else {
        // Show list of all available commands
        println!("Available commands:\n");
        print_all_commands();
        println!("\nUse 'dotstate help <command>' to see detailed help for a specific command.");
    }
    Ok(())
}

/// Execute the logs command.
pub fn cmd_logs() -> Result<()> {
    let log_dir = dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
        .join("dotstate");
    let log_file = log_dir.join("dotstate.log");
    println!("{}", log_file.display());
    Ok(())
}

/// Execute the config command.
pub fn cmd_config() -> Result<()> {
    let config_path = crate::utils::get_config_path();
    println!("{}", config_path.display());
    Ok(())
}

/// Execute the repository command.
pub fn cmd_repository() -> Result<()> {
    let repo_path = crate::utils::get_repository_path().context("Failed to get repository path")?;
    println!("{}", repo_path.display());
    Ok(())
}

/// Print all available commands with their descriptions.
pub fn print_all_commands() {
    let cli = Cli::command();
    let subcommands = cli.get_subcommands();

    for subcmd in subcommands {
        let name = subcmd.get_name();
        let about = subcmd
            .get_about()
            .map(std::string::ToString::to_string)
            .or_else(|| {
                subcmd
                    .get_long_about()
                    .map(std::string::ToString::to_string)
            })
            .unwrap_or_else(|| "No description available".to_string());

        // Format the command name with proper spacing
        let name_width = 15;
        let padded_name = if name.len() <= name_width {
            format!("{name:<name_width$}")
        } else {
            name.to_string()
        };

        println!("  {padded_name}{about}");

        // Print arguments/flags if any
        for arg in subcmd.get_arguments() {
            if let Some(short) = arg.get_short() {
                if let Some(long) = arg.get_long() {
                    let help = arg
                        .get_help()
                        .map_or_else(String::new, std::string::ToString::to_string);
                    println!("    -{short}, --{long:<12} {help}");
                }
            } else if let Some(long) = arg.get_long() {
                let help = arg
                    .get_help()
                    .map_or_else(String::new, std::string::ToString::to_string);
                println!("    --{long:<15} {help}");
            }
        }
    }
}
