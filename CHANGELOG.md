# Changelog

All notable changes to DotState will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Package manager feature with support for multiple package managers
- Profile-based package management
- Custom package support with user-defined install commands
- Real-time installation output streaming
- Check selected package feature
- Horizontal wrapping checkbox list for package manager selection
- Custom commit message support for `dotstate sync -m "message"`
- Automatic commit message generation from changed files (when no `-m` flag is provided)
- Unified commit logic for both CLI and TUI

### Changed
- Improved popup sizing for custom packages
- Enhanced package checking UX (removed auto-check on page load)
- Better error messages and user feedback
- Commit messages are now automatically generated from changed files instead of generic "Update dotfiles"
- Made existence check field optional for custom packages (if empty, uses standard binary name check)

### Fixed
- Fixed git clone failures caused by `.gitconfig` URL rewrites (e.g., `url."git@github.com:".insteadOf = "https://github.com/"`)
  - Token is now embedded directly in the URL to bypass gitconfig rewrites
  - Improved error messages to show underlying git2 errors with troubleshooting tips
  - Handle existing repositories more gracefully (reuse instead of failing)
- Fixed `dotstate logs` command showing incomplete path (now includes `dotstate.log` filename)

## [0.1.0] - 2025-01-XX

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

---

[Unreleased]: https://github.com/serkanyersen/dotstate/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/serkanyersen/dotstate/releases/tag/v0.1.0

## [0.1.1] - 2025-12-23

### Added
- Syntax Highlighting for file previews
- Added preview to sync changes page
- added website

## [0.1.2] - 2025-12-23

### Added
- homebrew tap

## [0.1.3] - 2025-12-23

### Added
- cargo publish workflow
- updated website instructions

