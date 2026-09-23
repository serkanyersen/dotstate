//! Omarchy desktop integration.
//!
//! Registers `DotState` as a launcher app on Omarchy that opens in a centered
//! floating window. Three artifacts, all under `$HOME`:
//!
//! 1. `~/.local/share/applications/dotstate.desktop`
//! 2. `~/.local/share/applications/icons/dotstate.png`
//! 3. A delimited block in the user's Hyprland config that tags the window
//!    `floating-window`, so Omarchy's own rules size and center it.
//!
//! Omarchy 3.x uses `~/.config/hypr/hyprland.conf`. Omarchy 4 (Quattro) moved
//! to Lua and uses `~/.config/hypr/hyprland.lua`. Omarchy's own trees
//! (`/usr/share/omarchy`, `~/.local/share/omarchy`) are never written to,
//! because updates overwrite them.

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The launcher script Omarchy uses to open or refocus a TUI.
const LAUNCHER: &str = "omarchy-launch-or-focus-tui";

/// Window class that `omarchy-launch-tui dotstate` assigns.
const APP_ID: &str = "org.omarchy.dotstate";

const ICON: &[u8] = include_bytes!("../../assets/dotstate-icon.png");

/// Which Hyprland config flavor the user has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HyprFlavor {
    /// Omarchy 3.x: `hyprland.conf`.
    Conf,
    /// Omarchy 4 (Quattro): `hyprland.lua`.
    Lua,
}

impl HyprFlavor {
    fn file_name(self) -> &'static str {
        match self {
            Self::Conf => "hyprland.conf",
            Self::Lua => "hyprland.lua",
        }
    }

    fn comment(self) -> &'static str {
        match self {
            Self::Conf => "#",
            Self::Lua => "--",
        }
    }

    fn rule(self) -> String {
        match self {
            Self::Conf => format!("windowrule = tag +floating-window, match:class {APP_ID}"),
            Self::Lua => format!(
                "o.window(\"^{}$\", {{ tag = \"+floating-window\" }})",
                APP_ID.replace('.', "\\\\.")
            ),
        }
    }

    fn start_marker(self) -> String {
        format!("{} >>> dotstate (omarchy) >>>", self.comment())
    }

    fn end_marker(self) -> String {
        format!("{} <<< dotstate (omarchy) <<<", self.comment())
    }

    fn block(self) -> String {
        format!(
            "\n{}\n{}\n{}\n",
            self.start_marker(),
            self.rule(),
            self.end_marker()
        )
    }
}

/// Paths the integration reads and writes, rooted at a home directory.
#[derive(Debug, Clone)]
pub struct OmarchyPaths {
    pub desktop_entry: PathBuf,
    pub icon: PathBuf,
    pub hypr_dir: PathBuf,
}

impl OmarchyPaths {
    #[must_use]
    pub fn new(home: &Path) -> Self {
        let apps = home.join(".local/share/applications");
        Self {
            desktop_entry: apps.join("dotstate.desktop"),
            icon: apps.join("icons/dotstate.png"),
            hypr_dir: home.join(".config/hypr"),
        }
    }

    fn hypr_config(&self, flavor: HyprFlavor) -> PathBuf {
        self.hypr_dir.join(flavor.file_name())
    }
}

/// Result of an install.
#[derive(Debug)]
pub struct InstallReport {
    pub flavor: HyprFlavor,
    pub hypr_config: PathBuf,
    pub reloaded: bool,
}

pub struct OmarchyService;

impl OmarchyService {
    /// Detect Omarchy and its Hyprland config flavor.
    ///
    /// Fails without writing anything when the Omarchy launcher is not on PATH
    /// or no Hyprland user config exists.
    pub fn detect(home: &Path) -> Result<HyprFlavor> {
        if !on_path(LAUNCHER) {
            bail!(
                "Omarchy not detected: `{LAUNCHER}` is not on PATH.\n\
                 This command only works on Omarchy (https://omarchy.org)."
            );
        }
        Self::detect_flavor(&OmarchyPaths::new(home))
    }

    fn detect_flavor(paths: &OmarchyPaths) -> Result<HyprFlavor> {
        // Quattro keeps a hyprland.conf around after migrating, so prefer Lua.
        if paths.hypr_config(HyprFlavor::Lua).exists() {
            Ok(HyprFlavor::Lua)
        } else if paths.hypr_config(HyprFlavor::Conf).exists() {
            Ok(HyprFlavor::Conf)
        } else {
            bail!(
                "No Hyprland user config found in {}.\n\
                 Expected hyprland.lua (Omarchy 4) or hyprland.conf (Omarchy 3).",
                paths.hypr_dir.display()
            )
        }
    }

    /// Install all artifacts. Safe to run repeatedly.
    pub fn install(home: &Path, flavor: HyprFlavor) -> Result<InstallReport> {
        let paths = OmarchyPaths::new(home);

        write_file(&paths.icon, ICON)?;
        write_file(&paths.desktop_entry, desktop_entry(&paths.icon).as_bytes())?;

        let hypr_config = paths.hypr_config(flavor);
        let content = fs::read_to_string(&hypr_config)
            .with_context(|| format!("Failed to read {}", hypr_config.display()))?;
        let updated = upsert_block(&content, flavor);
        if updated != content {
            fs::write(&hypr_config, updated)
                .with_context(|| format!("Failed to write {}", hypr_config.display()))?;
        }

        // After a 3.x -> Quattro upgrade, drop the rule we left in the old .conf.
        if flavor == HyprFlavor::Lua {
            strip_block_in_file(&paths.hypr_config(HyprFlavor::Conf), HyprFlavor::Conf)?;
        }

        Ok(InstallReport {
            flavor,
            hypr_config,
            reloaded: reload_hyprland(),
        })
    }

    /// Remove every artifact `install` created. Returns true if anything was removed.
    pub fn uninstall(home: &Path) -> Result<bool> {
        let paths = OmarchyPaths::new(home);
        let mut removed = false;
        for file in [&paths.desktop_entry, &paths.icon] {
            if file.exists() {
                fs::remove_file(file)
                    .with_context(|| format!("Failed to remove {}", file.display()))?;
                removed = true;
            }
        }
        for flavor in [HyprFlavor::Conf, HyprFlavor::Lua] {
            removed |= strip_block_in_file(&paths.hypr_config(flavor), flavor)?;
        }
        if removed {
            reload_hyprland();
        }
        Ok(removed)
    }
}

fn desktop_entry(icon: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Version=1.0\n\
         Name=DotState\n\
         Comment=Manage dotfiles with GitHub sync\n\
         Exec={LAUNCHER} dotstate\n\
         Terminal=false\n\
         Type=Application\n\
         Icon={}\n\
         Categories=Utility;System;\n\
         StartupNotify=true\n",
        icon.display()
    )
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    fs::write(path, bytes).with_context(|| format!("Failed to write {}", path.display()))
}

/// Remove every dotstate block, then append exactly one at the end.
fn upsert_block(content: &str, flavor: HyprFlavor) -> String {
    let mut out = remove_blocks(content, flavor);
    out.push_str(&flavor.block());
    out
}

/// Remove every dotstate block, including the newline `upsert_block` put before it.
/// `remove_blocks(upsert_block(x))` is always byte-identical to `x`.
fn remove_blocks(content: &str, flavor: HyprFlavor) -> String {
    let start = flavor.start_marker();
    let end = flavor.end_marker();
    let mut out = content.to_string();
    while let Some(s) = out.find(&start) {
        let Some(e_rel) = out[s..].find(&end) else {
            break; // Unterminated block: leave the file alone rather than guess.
        };
        let mut e = s + e_rel + end.len();
        if out[e..].starts_with('\n') {
            e += 1;
        }
        let s = if s > 0 && out.as_bytes()[s - 1] == b'\n' {
            s - 1
        } else {
            s
        };
        out.replace_range(s..e, "");
    }
    out
}

fn strip_block_in_file(path: &Path, flavor: HyprFlavor) -> Result<bool> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(false);
    };
    let stripped = remove_blocks(&content, flavor);
    if stripped == content {
        return Ok(false);
    }
    fs::write(path, stripped).with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(true)
}

/// Ask Hyprland to reload its config. Best effort: returns false if it failed.
fn reload_hyprland() -> bool {
    if cfg!(test) {
        return false;
    }
    Command::new("hyprctl")
        .arg("reload")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn on_path(binary: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(binary).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const LUA_SKELETON: &str = "require(\"default.hypr.omarchy\")\n\n\
        -- Add any other personal Hyprland configuration below.\n\
        -- o.window(\"qemu\", { workspace = \"5\" })\n";

    #[test]
    fn lua_rule_escapes_dots() {
        assert_eq!(
            HyprFlavor::Lua.rule(),
            r#"o.window("^org\\.omarchy\\.dotstate$", { tag = "+floating-window" })"#
        );
    }

    #[test]
    fn block_round_trips_byte_identical() {
        for flavor in [HyprFlavor::Conf, HyprFlavor::Lua] {
            for original in ["", "a\n", "a", "a\n\n", LUA_SKELETON] {
                let installed = upsert_block(original, flavor);
                assert_eq!(installed.matches(&flavor.start_marker()).count(), 1);
                assert_eq!(remove_blocks(&installed, flavor), original);
            }
        }
    }

    #[test]
    fn upsert_is_idempotent_and_replaces_stale_or_duplicate_blocks() {
        let flavor = HyprFlavor::Conf;
        let once = upsert_block("x\n", flavor);
        assert_eq!(upsert_block(&once, flavor), once);

        let stale = once.replace(&flavor.rule(), "windowrule = old");
        assert_eq!(upsert_block(&stale, flavor), once);

        let twice = format!("{once}{}", flavor.block());
        assert_eq!(upsert_block(&twice, flavor), once);
    }

    #[test]
    fn unterminated_block_is_left_alone() {
        let content = format!("x\n{}\nrule\n", HyprFlavor::Conf.start_marker());
        assert_eq!(remove_blocks(&content, HyprFlavor::Conf), content);
    }

    #[test]
    fn detect_prefers_lua_and_fails_without_config() {
        let home = TempDir::new().unwrap();
        let paths = OmarchyPaths::new(home.path());
        assert!(OmarchyService::detect_flavor(&paths).is_err());

        fs::create_dir_all(&paths.hypr_dir).unwrap();
        fs::write(paths.hypr_config(HyprFlavor::Conf), "").unwrap();
        assert_eq!(
            OmarchyService::detect_flavor(&paths).unwrap(),
            HyprFlavor::Conf
        );

        fs::write(paths.hypr_config(HyprFlavor::Lua), "").unwrap();
        assert_eq!(
            OmarchyService::detect_flavor(&paths).unwrap(),
            HyprFlavor::Lua
        );
    }

    #[test]
    fn install_reinstall_uninstall_on_quattro_after_upgrade() {
        let home = TempDir::new().unwrap();
        let paths = OmarchyPaths::new(home.path());
        fs::create_dir_all(&paths.hypr_dir).unwrap();
        let lua = paths.hypr_config(HyprFlavor::Lua);
        let conf = paths.hypr_config(HyprFlavor::Conf);
        // A 3.x install left its rule in hyprland.conf before the upgrade.
        let old_conf = "source = foo\n";
        fs::write(&conf, upsert_block(old_conf, HyprFlavor::Conf)).unwrap();
        fs::write(&lua, LUA_SKELETON).unwrap();

        OmarchyService::install(home.path(), HyprFlavor::Lua).unwrap();
        let after_first = fs::read_to_string(&lua).unwrap();
        assert!(after_first.contains(&HyprFlavor::Lua.rule()));
        assert_eq!(fs::read_to_string(&conf).unwrap(), old_conf);
        assert_eq!(fs::read(&paths.icon).unwrap(), ICON);
        let entry = fs::read_to_string(&paths.desktop_entry).unwrap();
        assert!(entry.contains("Exec=omarchy-launch-or-focus-tui dotstate"));
        assert!(entry.contains(&format!("Icon={}", paths.icon.display())));

        OmarchyService::install(home.path(), HyprFlavor::Lua).unwrap();
        assert_eq!(fs::read_to_string(&lua).unwrap(), after_first);

        assert!(OmarchyService::uninstall(home.path()).unwrap());
        assert_eq!(fs::read_to_string(&lua).unwrap(), LUA_SKELETON);
        assert!(!paths.desktop_entry.exists());
        assert!(!paths.icon.exists());
        assert!(!OmarchyService::uninstall(home.path()).unwrap());
    }
}
