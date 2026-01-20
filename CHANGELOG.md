# Changelog

All notable changes to DotState will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Package Manager**: Automatically prompt to install newly added packages if they are not already installed on the system
- **Package Manager**: Import packages from package managers (Homebrew, Pacman, APT, DNF, Yum, Snap, Cargo, npm, pip, gem) with `Shift+I`. Features tabbed interface to switch between sources with `←/→`, multi-select with filtering, async discovery with spinner animation, and per-source caching (5 minutes)

### Changed
- **Package Manager**: Deleted packages are now removed from the status cache
- **Package Manager**: Newly added packages always get a fresh status check, ignoring any stale cache entries

### Fixed
- **Sync**: Fixed sync creating unnecessary merge commits when pulling remote changes. Now uses proper git rebase instead of merge, resulting in a clean linear commit history
- **Sync**: Fixed "cannot push non-fastforwardable reference" error after rebase by properly updating the branch reference to point to the rebased HEAD
- **Package Manager**: Fixed race condition where successful package installations were incorrectly reported as both success and failure with "Installation thread disconnected" error
- **Package Manager**: Fixed installation dialogs rendering without background dimming by ensuring main content is rendered first
- **Package Manager**: Fixed validation errors showing as full-screen dialogs instead of inline in the add/edit popup
- **Package Manager**: Prevent duplicate packages by validating binary name uniqueness
- **Package Manager**: Fixed import popup showing blank screen during discovery by deferring discovery until after loading UI renders
- **Package Manager**: Fixed Space key being captured by filter input in import popup instead of toggling package selection
- **Keymap**: Fixed Shift+key bindings not working on terminals that send uppercase characters without SHIFT modifier

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
