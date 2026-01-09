# Changelog

All notable changes to DotState will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
