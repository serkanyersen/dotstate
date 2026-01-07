# DotState

> **A modern, secure, and user-friendly dotfile manager built with Rust**

DotState is a terminal-based tool that helps you manage your dotfiles effortlessly. Whether you're syncing your configuration across multiple machines or setting up a new development environment, DotState makes it simple, safe, and fast.

## Demo
https://github.com/user-attachments/assets/9be0df5e-87ce-4b61-ae0f-1c8ffe94cb36

## Why DotState?

Managing dotfiles can be a pain. You want your `.zshrc`, `.vimrc`, and other config files synced across machines, but traditional solutions are either too complex, insecure, or require too much manual work.

**DotState solves this by being:**
- 🦀 **Built with Rust** - Fast, memory-safe, and reliable
- 🔒 **Secure by design** - No shell injection vulnerabilities, safe file operations
- 🎨 **Beautiful TUI** - Intuitive interface that doesn't require learning Git
- ⚡ **Lightning fast** - Non-blocking operations, instant feedback
- 🛡️ **Safe** - Automatic backups before any file operations
- 🔄 **GitHub-powered** - Your dotfiles stored securely in a private or public repo

## What Makes DotState Different?

### Traditional Dotfile Managers
- Require Git knowledge
- Manual symlink management
- No built-in backup system
- Complex setup process

### DotState
- **Zero Git knowledge required** - We handle everything
- **Automatic symlink management** - Files are linked automatically
- **Built-in backups** - Your files are safe before any operation
- **One-command setup** - Get started in seconds
- **Profile support** - Separate configs for work, personal, Mac, Linux, etc.
- **Package management** - Track and install CLI tools per profile
- **Beautiful TUI** - Visual interface with mouse support

## Features

### 🎯 Core Features

- **Profile Management**: Create separate profiles for different contexts (work, personal, Mac, Linux, etc.)
- **GitHub Sync**: Automatic sync with GitHub (private or public repos)
- **Smart File Detection**: Automatically finds common dotfiles in your home directory
- **Safe Operations**: Automatic backups before any file modification
- **Symlink Management**: Automatic creation and management of symlinks
- **Custom Files**: Add any file or directory, not just dotfiles

### 📦 Package Management

- **CLI Tool Tracking**: Define and track CLI tools and dependencies per profile
- **Multi-Manager Support**: Works with Homebrew, Cargo, npm, pip, and more
- **Installation Flow**: Check what's missing and install with one command
- **Custom Packages**: Support for custom installation scripts

### 🎨 User Experience

- **Beautiful TUI**: Modern terminal interface built with Ratatui
- **Mouse Support**: Click to navigate and interact
- **Real-time Feedback**: See what's happening as it happens
- **Error Recovery**: Clear error messages with actionable guidance
- **CLI & TUI**: Full-featured CLI for automation, beautiful TUI for interactive use

### 🔒 Security

- **No Shell Injection**: Direct command execution, no shell interpretation
- **Safe File Operations**: Validates paths, prevents dangerous operations
- **Secure GitHub Integration**: Token-based authentication
- **Backup System**: Automatic backups before any destructive operation

## Installation

### Prebuilt from website (Recommended)
[Installation Guide](https://dotstate.serkan.dev/#installation)
```bash
/bin/bash -c "$(curl -fsSL https://dotstate.serkan.dev/install.sh)"
```

### Using Cargo

```bash
cargo install dotstate
```

### Using Homebrew

```bash
brew tap serkanyersen/dotstate
brew install dotstate
```

Or use the direct install:
```bash
brew install serkanyersen/dotstate/dotstate
```

## Quick Start

1. **Launch DotState**:
   ```bash
   dotstate
   ```

2. **First-time Setup**:
   - Enter your GitHub token (create one at [github.com/settings/tokens](https://github.com/settings/tokens))
     - **Tip**: You can also set the `DOTSTATE_GITHUB_TOKEN` environment variable instead of entering it in the TUI
   - Choose repository name and location
   - Select repository visibility (private/public)

3. **Add Your Files**:
   - Navigate to "Manage Files"
   - Select files to sync (they're automatically added)
   - Files are moved to the repo and symlinked automatically

4. **Sync to GitHub**:
   - Go to "Sync with Remote"
   - Your files are committed, pulled, and pushed automatically

That's it! Your dotfiles are now synced and ready to use on any machine.

## CLI Usage

DotState also provides a powerful CLI for automation:

```bash
# List all synced files
dotstate list

# Add a file to sync
dotstate add ~/.myconfig

# Sync with remote (commit, pull, push)
dotstate sync

# Activate symlinks (useful after cloning on a new machine)
dotstate activate

# Deactivate symlinks (restore original files)
dotstate deactivate

# Show help
dotstate help
```

## How It Works

1. **Storage**: Your dotfiles are stored in a Git repository (default: `~/.config/dotstate/storage`)
2. **Symlinks**: Original files are replaced with symlinks pointing to the repo
3. **Profiles**: Different profiles can have different sets of files
4. **Sync**: Changes are committed and synced with GitHub automatically

## Configuration

### GitHub Token

DotState supports two ways to provide your GitHub token:

1. **Environment Variable** (Recommended for automation):
   ```bash
   export DOTSTATE_GITHUB_TOKEN=ghp_your_token_here
   ```
   The environment variable takes precedence over the config file token.

2. **Config File**: The token can be stored in the config file (set during first-time setup).

**Note**: If `DOTSTATE_GITHUB_TOKEN` is set, it will be used automatically, and the token in the config file becomes optional. This is useful for:
- CI/CD pipelines
- Shared machines where you don't want to store tokens in config files
- Using different tokens for different environments

## Security Considerations

- **No Shell Injection**: All commands use direct execution, not shell interpretation
- **Path Validation**: Dangerous paths (like home directory root) are blocked
- **Git Repository Detection**: Prevents nested Git repositories
- **Backup System**: Automatic backups before any file operation
- **Token Security**: GitHub tokens can be provided via `DOTSTATE_GITHUB_TOKEN` environment variable (recommended) or stored in config files with secure permissions

## Requirements

- **Rust**: Latest stable version (for building from source)
- **Git**: For repository operations
- **GitHub Account**: For cloud sync (optional, but recommended)

## Project Status

DotState is actively developed and ready for use. The core features are stable, and we're continuously improving based on user feedback.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

Built with:
- [Ratatui](https://github.com/ratatui-org/ratatui) - Beautiful TUI framework
- [git2](https://github.com/rust-lang/git2-rs) - Git operations
- [clap](https://github.com/clap-rs/clap) - CLI parsing

Badges:

<a title="This tool is Tool of The Week on Terminal Trove, The $HOME of all things in the terminal" href="https://terminaltrove.com/dotstate/"><img width="180" src="https://cdn.terminaltrove.com/media/badges/tool_of_the_week/svg/terminal_trove_tool_of_the_week_green_on_dark_grey_bg.svg" alt="Terminal Trove Tool of The Week" /></a>

## Support

- **Issues**: [GitHub Issues](https://github.com/serkanyersen/dotstate/issues)
- **Discussions**: [GitHub Discussions](https://github.com/serkanyersen/dotstate/discussions)

---

**Made with ❤️ and Rust**
