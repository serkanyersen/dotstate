//! Doctor command for running diagnostics.

use crate::config::Config;
use crate::utils::doctor::{Doctor, DoctorOptions};
use anyhow::{Context, Result};

/// Execute the doctor command.
pub fn execute(fix: bool, verbose: bool, json: bool) -> Result<()> {
    let config_path = crate::utils::get_config_path();
    let config = Config::load_or_create(&config_path).context("Failed to load configuration")?;

    if !config.is_repo_configured() {
        if json {
            println!(
                r#"{{"error": "Repository not configured", "suggestion": "Run 'dotstate' to set up repository"}}"#
            );
        } else {
            eprintln!("❌ Repository not configured. Please run 'dotstate' to set up repository.");
        }
        std::process::exit(1);
    }

    let options = DoctorOptions {
        fix_mode: fix,
        verbose,
        json_output: json,
    };

    let mut doctor = Doctor::new(config, options);
    let report = doctor.run_diagnostics()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    // Summary is printed by doctor itself when not in json mode

    if report.summary.errors > 0 {
        std::process::exit(1);
    }

    Ok(())
}
