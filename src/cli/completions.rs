//! Completions command for generating shell completions.

use crate::cli::Cli;
use anyhow::bail;
use clap::CommandFactory;
use clap_complete::Shell;

/// Generate command-line completions.
pub fn generate(shell: Option<Shell>) -> std::result::Result<(), anyhow::Error> {
    let Some(shell) = shell.or_else(Shell::from_env) else {
        bail!("Could not automatically detect shell");
    };

    let mut cmd = Cli::command();
    let name = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, name, &mut std::io::stdout());

    Ok(())
}
