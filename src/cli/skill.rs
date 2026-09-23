//! `dotstate --skill`: print or install the built-in agent skill.
//!
//! The skill is embedded at build time so it always matches the installed binary.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

/// The agent skill, in `SKILL.md` format.
pub const SKILL: &str = include_str!("skill.md");

/// Outcome of installing the skill.
#[derive(Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed(PathBuf),
    Updated(PathBuf),
    UpToDate(PathBuf),
}

/// Where the skill is installed, relative to the home directory.
fn skill_path(home: &Path) -> PathBuf {
    home.join(".claude")
        .join("skills")
        .join("dotstate")
        .join("SKILL.md")
}

/// Write the skill under `home`. Re-running replaces the file only if its content changed.
pub fn install(home: &Path) -> Result<InstallOutcome> {
    let path = skill_path(home);
    let existing = fs::read_to_string(&path).ok();
    if existing.as_deref() == Some(SKILL) {
        return Ok(InstallOutcome::UpToDate(path));
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    fs::write(&path, SKILL).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(if existing.is_some() {
        InstallOutcome::Updated(path)
    } else {
        InstallOutcome::Installed(path)
    })
}

/// Execute `dotstate --skill [--install]`.
pub fn execute(install_skill: bool) -> Result<()> {
    if !install_skill {
        print!("{SKILL}");
        return Ok(());
    }
    let home = dirs::home_dir().context("Failed to get home directory")?;
    match install(&home)? {
        InstallOutcome::Installed(path) => {
            println!("Installed DotState skill to {}", path.display());
        }
        InstallOutcome::Updated(path) => println!("Updated DotState skill at {}", path.display()),
        InstallOutcome::UpToDate(path) => {
            println!("DotState skill is already up to date at {}", path.display());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn skill_has_frontmatter() {
        assert!(SKILL.starts_with("---\nname: dotstate\ndescription: "));
    }

    #[test]
    fn install_is_idempotent_and_updates_stale_copies() {
        let home = TempDir::new().unwrap();
        let path = skill_path(home.path());

        assert_eq!(
            install(home.path()).unwrap(),
            InstallOutcome::Installed(path.clone())
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL);

        assert_eq!(
            install(home.path()).unwrap(),
            InstallOutcome::UpToDate(path.clone())
        );

        fs::write(&path, "old").unwrap();
        assert_eq!(
            install(home.path()).unwrap(),
            InstallOutcome::Updated(path.clone())
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), SKILL);
    }
}
