# Changelog

All notable changes to DotState will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Mouse**: Comprehensive mouse support across all screens — click to select items, click to focus panes, scroll wheel navigates lists and previews
- **Mouse**: `MouseRegions<T>` utility for consistent hit-testing of clickable areas
- **Mouse**: File browser component supports click-to-focus path input, file list, and preview panes
- **Mouse**: Popup dialogs support click-to-focus fields — Profile Create (Name/Description/CopyFrom), Package Add/Edit (all fields), Package Import (tabs/filter/list with scroll)
- **Themes**: Gruvbox Dark and Light themes - warm, retro colors popular with Vim/Neovim users
- **Themes**: Catppuccin Mocha (dark) and Latte (light) themes - pastel aesthetic
- **Themes**: Tokyo Night (dark) and Light themes - modern city-lights inspired palette

### Changed

- **Settings**: Renamed "Embed Credentials in URL" to "Token in Remote URL" with clearer description explaining the two authentication modes
- **Settings**: Hide "Token in Remote URL" option when repository is in local mode (tokens are only relevant for GitHub repos)
- **Themes**: Simplified theme cycling and config string logic to use `ThemeType::all()` and `to_config_string()`

---

## [0.2.25] - 2026-02-04

### Fixed

- **Git SSH**: Use system git for SSH remote operations (fetch, push, pull, clone) to fix compatibility with 1Password SSH Agent, YubiKey Agent, Secretive, and other non-standard SSH agent implementations. HTTPS operations continue to use git2 as before.

---

## [0.2.24] - 2026-02-03

### Added

- **Dialogs**: Scrollable dialogs with keyboard navigation (↑↓/jk, PageUp/PageDown, Home) for viewing long content

### Fixed

- **Git Sync**: Improved recovery from failed sync operations - commit is automatically undone on push failure to preserve staged changes
- **Git Sync**: Better error messages for push rejections - now captures and displays full git hook output

---

## [0.2.23] - 2026-02-02

### Added

- **Settings**: New "Embed Credentials in URL" toggle - allows disabling credential embedding in git remote URLs for environments that reject URLs with embedded tokens

### Changed

- **Profile Selection**: Converted from full-screen to popup dialog during onboarding with inline profile creation and preview panel

### Fixed

- **Profile Selection**: Fixed text input where global keybindings interfered with typing
- **Git Sync**: Fixed silent push failures - server-side rejections are now properly captured and reported

---

## [0.2.22] - 2026-02-01

### Added

- **File Versioning**: Schema versioning for all config/data files with automatic migration on load. Files without a version field are treated as v0 and auto-migrated with backup safety.
- **Atomic Saves**: All config/data file saves now use atomic write (temp file + rename) to prevent corruption on crash.

### Fixed

- **Package Installation**: Fixed Homebrew on custom installations, CLI output streaming, TUI responsiveness during install, and auto-scroll behavior
- **Storage Setup**: Fixed GitHub setup progress popup being clipped
- **Packages CLI**: Fixed `packages list` and `packages check` not populating cache
- **Main Menu**: Fixed "requires setup" check using proper `is_repo_configured()`

---

## [0.2.21] - 2025-01-30

### Fixed

- **Storage Setup**: Fixed setup flow issues affecting both GitHub and local repo modes - async step handling no longer hangs, and "already configured" check now works correctly

---

## [0.2.20] - 2025-01-29

### Added

- **Code Quality**: Comprehensive clippy lint configuration with pedantic lints enabled

### Changed

- **Main Menu**: Selection now persists when navigating back from sub-screens

### Fixed

- **Dotfile Selection**: Added confirmation dialog when removing common files from sync
- **Stability**: Theme system recovers gracefully from RwLock poison instead of panicking
- **Stability**: Fixed potential race conditions in symlink metadata checks

---

## [0.2.19] - 2025-01-27

### Added

- **CLI**: `dotstate packages` subcommand for managing packages from command line
  - `packages list` - List packages with installation status
  - `packages add` - Add packages interactively or via flags
  - `packages remove` - Remove packages by name or selection
  - `packages check` - Check installation status
  - `packages install` - Install all missing packages
  - All commands support `--profile` flag for non-active profiles
- **CLI**: Modular CLI architecture with `src/cli/` folder structure

### Changed

- **App Architecture**: Major refactor of `app.rs` - reduced from ~2400 to ~1400 lines (40% reduction)
- **Screens**: Screens now handle their own actions via `process_action()` methods with type-safe action enums

---

## [0.2.18] - 2025-01-25

### Added

- **Storage Setup**: New unified storage setup screen with async GitHub flow, visual progress tracking, and edit token functionality in reconfiguration mode

### Fixed

- **Deactivate**: Fixed crash when deactivating directories containing symlinks; restore errors are now warnings instead of fatal
- **Storage Setup**: Improved error recovery - config resets to unconfigured state on failure, empty repos get default profile created
- **Storage Setup**: Fixed reconfiguration mode - Enter no longer triggers re-setup, disabled fields block input, token updates work with scoped tokens
- **Storage Setup**: Fixed global keybindings (q, t) being triggered while typing in text fields
- **Storage Setup**: Added support for fine-grained tokens (`github_pat_`) with detailed permission help text
- **Sync**: Fixed sync not detecting modified files (only untracked files were being detected)
- **Git**: Improved push error messages with token permission guidance for classic and fine-grained tokens
- **Documentation**: Updated README with detailed GitHub token setup guide

### Removed

- **GitHub Auth Screen**: Removed legacy screen, replaced by new `StorageSetupScreen`

---

## [0.2.17] - 2025-01-23

### Added

- **Dotfile Candidates**: Added AI tool configurations as sync candidates

### Changed

- **Installer**: Revamped install script with version detection, auto-PATH configuration, CLI flags, and ASCII banner

---

## [0.2.16] - 2025-01-21

### Fixed

- **Keymap**: Fixed vim keybindings `h` and `l` being captured as cursor movement instead of typing characters in text input fields (Fixes #39)

---

## [0.2.15] - 2025-01-21

### Fixed

- **Package Manager**: Fixed installation progress counter off-by-one error and added auto-scroll to output window
- **Package Manager**: Fixed app freeze with custom `sudo` commands (checks password requirement, closes stdin for interactive commands)
- **Package Manager**: Package check now shows detailed check attempts when package is not found

---

## [0.2.14] - 2025-01-21

### Fixed

- **Sync with Remote**: Fixed local repo mode to work without token and results popup now closes correctly with Enter key
- **Manage Files**: Fixed common files handling (warning dialog when trying to unsync, proper file marking when re-entering screen)

---

## [0.2.13] - 2025-01-21

### Added

- **UI**: Toast notification system for non-blocking success confirmations (auto-dismiss after 3 seconds)

### Changed

- **UI**: Error messages now display as centered modal dialogs instead of full-screen overlays
- **Manage Files**: Preview pane is now always visible, improving layout consistency
- **Package Manager**: Import tabs now use ratatui's built-in Tabs widget for consistent styling

### Fixed

- **Move to Common**: Fixed bug where confirming deletion of files with different content in other profiles did not actually delete them

---

## [0.2.12] - 2025-01-20

### Fixed

- **Sync Validation**: Added symlink validation before adding directories to sync, preventing crashes from broken, circular, or problematic symlinks

---

## [0.2.11] - 2025-01-20

### Added

- **Package Manager**: Import packages from installed package managers with `Shift+I`. Supports Homebrew, Pacman, APT, DNF, Yum, Snap, Cargo, npm, pip, and gem with a tabbed interface, multi-select filtering, and result caching
- **Package Manager**: Prompt to install newly added packages if not already installed

### Changed

- **Sync**: Now uses git rebase instead of merge when pulling remote changes, resulting in cleaner linear commit history
- **Package Manager**: Improved status cache management for added/deleted packages

### Fixed

- **Sync**: Fixed various issues with rebase workflow (branch reference updates, detached HEAD state)
- **Package Manager**: Fixed installation reliability and UI rendering issues
- **Keymap**: Fixed Shift+key bindings on terminals that send uppercase characters

---

## [0.2.10] - 2025-01-19

### Changed

- **CLI**: Enhanced doctor command with comprehensive diagnostics:
  - Added version check (shows if updates are available)
  - Added 7 diagnostic categories: Environment, Configuration, Repository, Profiles, Symlinks, Backups, Filesystem
  - Added `--verbose` flag for detailed output with file paths and extra info
  - Added `--json` flag for machine-readable output (scripting support)
  - Colored output with timing per check and summary statistics
  - Fixed backup check to read actual config setting and correct backup location (~/.dotstate-backups)

### Security

- **Git URLs**: Credentials/tokens are now redacted from git remote URLs in all output (doctor, error messages, UI displays)

### Fixed

- **Sync Service**: Fixed `move_to_common` and `move_from_common` to use SymlinkManager instead of raw symlinks, ensuring proper symlink tracking

## [0.2.9] - 2026-01-19

### Added

- **CLI**: Add doctor command to check for issues with the dotstate configuration

### Fixed

- **Package Manager**: Only update packages in the manage packages screen if the active profile has changed.
- **Setup**: Fix an issue where common files were not created after setup.
- **CLI**: Fix an issue where activate/deactivate commands were not working properly.
- **Symlinks**: Fix an issue where symlinks tracking could go out of sync.
- **UI**: Unified some more UI components

## [0.2.8] - 2026-01-17

### Added

- **Settings**: Added new settings page with options to configure keymap, icon set, backups, and updates
- **Theme**: Added new themes: Solarized, Solarized Dark, and Midnight

### Changed

- **Theme**: Updated color definitions to be more consistent

## [0.2.7] - 2026-01-17

### Added

- **Theme**: Added new fixed theme with unified colors

### Fixed

- **Keymap**: Fixed keyhandling issues when inputs are focused

### Changed

- **Theme**: Updated color definitions to be more consistent

## [0.2.6] - 2026-01-16

### Added

- **Package Check Status**: Show the status of package checks in the UI and remember the status for each package
- **File Manager**: Show details of the folder contents in the file manager

### Changed

- **Theme**: Unify border styles and add to themes

### Fixed

- **Package Manager**: Fix check all packages command

---

## [0.2.5] - 2026-01-16

### Added

- **Common Files Support**: Core implementation for shared dotfiles that persist across multiple profiles
- **Improved File Management**:
  - New 'Move' action with dedicated keybindings for better file organization
  - Dialog-based validation and confirmation when moving files to the common profile
- **Sync Enhancements**:
  - Automatic management of common file symlinks after remote sync operations
  - Sync service now detects and includes files from the manifest that are missing in the local configuration
  - Integration of profile symlink verification into the sync process for better consistency
- **UI Improvements**:
  - Visual error display during profile creation for better feedback
  - Standardized title padding across all screens

### Changed

- **UI Component Standardization**:
  - Standardized all Popups and Dialogs (Delete, Switch, Create, Rename) for a uniform look and feel
  - Improved popup rendering with footers now correctly placed inside borders
- **GitHub Authentication**: Refactored `GitHubAuthScreen` for improved rendering and more robust event handling

### Removed

- **ViewSyncedFiles Screen**: Removed the redundant "View Synced Files" screen to streamline the user flow

---

## [0.2.4] - 2026-01-14

### Added

- **Configurable Keymap System**: Complete refactor of keyboard command handling to use a configurable keymap system
  - Three preset keymaps: Standard (arrow keys), Vim (hjkl), and Emacs (Ctrl+N/P)
  - Custom key binding overrides - override any action with any key combination
  - Override shadowing - when an action is overridden, preset bindings for that action are automatically removed
  - Dynamic footer display - UI footers automatically reflect actual key bindings including overrides
  - Help overlay (press `?`) shows all current key bindings based on your configuration
  - Keymap configuration stored in `~/.config/dotstate/config.toml` with TOML format
  - Example configuration file: `examples/keymap_override_example.toml`
  - All screens and components migrated to use keymap actions instead of hardcoded keys
  - Support for modifier keys in overrides (e.g., `ctrl+h`, `ctrl+shift+j`)
  - Support for special keys in overrides (e.g., `f1`, `enter`, `esc`, `tab`)

- **Enhanced Git Sync Status**: Detailed tracking of ahead/behind counts and pending changes
- **Icon System**: Native support for NerdFonts, Emojis, and ASCII icons with configurable settings
- **New Main Menu**: Redesigned main menu interface for better usability
- **Enhanced Status UI**: Improved detailed Git status display for sync operations

### Changed

- **Keyboard Event Handling**: All keyboard commands now use the configurable keymap system instead of hardcoded key checks
- **Component Event Handling**: Components now use keymap actions instead of hardcoded key codes
- **Footer Display**: Footers dynamically show actual key bindings based on current keymap configuration
- **Help Overlay**: Help overlay now displays all bindings from keymap (preset + overrides) instead of hardcoded values
- **Architectural Refactor**: Major migration to a Screen-based architecture for better state management and UI responsiveness
- **Text Input Handling**: Unified and improved text input behavior across all screens
- **Sync Efficiency**: Improved efficiency of file syncing operations
- **Performance**: Optimized profile checks to reduce IO operations
- **UI Components**: Refactored UI components for consistency and better performance

### Fixed

- Removed redundant hardcoded key fallbacks that were no longer needed after keymap migration
- Fixed component event handling to properly use keymap system
- Fixed display functions to reflect actual key bindings instead of preset-only values
- **Keymap Display**: Fixed display issues for keymaps and footers
- **Profile Selection**: Fixed bug with initial profile selection logic

---

## [0.2.2] - 2026-01-09

### Added

- Robust integration tests to catch bugs related to syncing early on

### Fixed

- Users could add nested files or nested .git folder, which would cause the app to crash before completing the sync
- main menu default selection fixed.

## [0.2.1] - 2026-01-08

### Fixed

- **Universal Linux Binaries**: Switched from glibc to musl static linking for Linux builds
  - Fixes GLIBC version errors (`GLIBC_2.38 not found`, `GLIBC_2.39 not found`) on older systems
  - Binaries now work on any Linux distribution regardless of glibc version
  - Tested on Ubuntu 22.04/20.04, Debian 11, Alpine, CentOS 7, and Amazon Linux 2
  - Resolves [#12](https://github.com/serkanyersen/dotstate/issues/12)

### Changed

- Release workflow now uses `cross` tool with musl targets for Linux builds
- Install script updated to download musl binaries (`*-linux-musl` instead of `*-linux-gnu`)

## [0.2.0] - 2026-01-07

### Added

- **Local Repository Mode**: Use your own git repository instead of having DotState create one via GitHub
  - Support for any git host (GitHub, GitLab, Bitbucket, self-hosted, etc.)
  - Uses system git credentials (SSH keys, git credential manager) - no token required
  - New setup mode selection screen on initial setup
  - Validation for local repos (checks for .git directory and origin remote)
- **Update Notifications**: DotState now checks for updates and notifies you when a new version is available
  - Update notification banner in main menu
  - New `dotstate upgrade` CLI command with interactive options
  - Configurable check interval and ability to disable update checks
  - Multiple update methods: install script, cargo, or homebrew
- **Theme System**: Comprehensive theme support for light and dark terminal backgrounds
  - Automatic theme detection and adaptation
  - Light and dark themes with consistent color palette
  - Syntax highlighting themes automatically match UI theme (light/dark)
  - All UI elements (headers, footers, borders, text, lists) use theme colors
  - Configurable via `theme` setting in config file (`"dark"` or `"light"`)
  - `--no-colors` CLI flag to disable colors entirely
  - `theme = "nocolor"` to disable all UI colors (same as `NO_COLOR=1`)
- Custom commit message support for `dotstate sync -m "message"`
- Automatic commit message generation from changed files (when no `-m` flag is provided)
- Unified commit logic for both CLI and TUI

### Changed

- Renamed "Setup GitHub Repository" to "Setup git repository" in main menu
- Updated menu explanation to describe both setup options (GitHub vs Local)
- Sync operations now work without token in Local mode
- CLI commands updated to support Local mode
- Improved popup sizing for custom packages
- Enhanced package checking UX (removed auto-check on page load)
- Better error messages and user feedback
- Commit messages are now automatically generated from changed files instead of generic "Update dotfiles"
- Made existence check field optional for custom packages (if empty, uses standard binary name check)
- **UI Color System**: All hardcoded colors replaced with theme-based system
  - Consistent color usage across all components
  - Better visibility in both light and dark terminals
  - All borders, headers, footers, and text now respect theme settings

### Fixed

- Fixed git clone failures caused by `.gitconfig` URL rewrites (e.g., `url."git@github.com:".insteadOf = "https://github.com/"`)
  - Token is now embedded directly in the URL to bypass gitconfig rewrites
  - Improved error messages to show underlying git2 errors with troubleshooting tips
  - Handle existing repositories more gracefully (reuse instead of failing)
- Fixed `dotstate logs` command showing incomplete path (now includes `dotstate.log` filename)

## [0.1.3] - 2025-12-23

### Added

- cargo publish workflow
- updated website instructions

## [0.1.2] - 2025-12-23

### Added

- homebrew tap

## [0.1.1] - 2025-12-23

### Added

- Syntax Highlighting for file previews
- Added preview to sync changes page
- added website

## [0.1.0] - 2025-01-22

### Added

- Initial release
- TUI interface for managing dotfiles
- GitHub sync functionality
- Profile management (multiple profiles support)
- Automatic symlink management
- Backup system before file operations
- CLI commands for automation
- File browser for custom file selection
- Package manager integration
- Real-time installation progress
- Mouse support in TUI

### Security

- No shell injection vulnerabilities
- Safe path validation
- Git repository detection to prevent nested repos
