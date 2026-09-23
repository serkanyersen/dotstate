//! `dotstate omarchy`: Omarchy desktop integration.

use anyhow::{Context, Result};

use crate::services::omarchy_service::{HyprFlavor, OmarchyService};

/// Execute `dotstate omarchy --install` or `--uninstall`.
pub fn execute(install: bool) -> Result<()> {
    let home = dirs::home_dir().context("Failed to get home directory")?;

    if !install {
        if OmarchyService::uninstall(&home)? {
            println!("✅ Removed the DotState launcher entry, icon, and window rule");
        } else {
            println!("ℹ️  Nothing to remove");
        }
        return Ok(());
    }

    let flavor = match OmarchyService::detect(&home) {
        Ok(flavor) => flavor,
        Err(e) => {
            eprintln!("❌ {e}");
            std::process::exit(1);
        }
    };
    let report = OmarchyService::install(&home, flavor)?;
    let version = match report.flavor {
        HyprFlavor::Lua => "Omarchy 4",
        HyprFlavor::Conf => "Omarchy 3",
    };
    println!("✅ DotState is registered as an app ({version})");
    println!("   Window rule: {}", report.hypr_config.display());
    if !report.reloaded {
        println!("   ⚠️  `hyprctl reload` failed. Reload Hyprland to apply the window rule.");
    }
    println!("   Launch it from the app launcher (SUPER + SPACE).");
    Ok(())
}
