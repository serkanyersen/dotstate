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
- 🔄 **Git-powered** - Store dotfiles in GitHub, GitLab, Bitbucket, or any git host

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
- **Flexible Git Sync**: Automatic sync with GitHub, GitLab, Bitbucket, or any git host
- **Two Setup Modes**: Let DotState create a GitHub repo for you, or use your own repository
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
- **Customizable Keymaps**: Configurable keyboard shortcuts with preset support (Standard, Vim, Emacs) and custom overrides

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
   - Choose how to set up your repository:
     - **Option A: Create for me (GitHub)** - DotState creates a repo on GitHub
       - Enter your GitHub token (create one at [github.com/settings/tokens](https://github.com/settings/tokens))
       - **Tip**: You can also set the `DOTSTATE_GITHUB_TOKEN` environment variable
       - Choose repository name and visibility (private/public)
     - **Option B: Use my own repository** - Bring your own git repo
       - Create a repo on any git host (GitHub, GitLab, Bitbucket, etc.)
       - Clone it locally and set up your credentials
       - Point DotState to your local repo path

3. **Add Your Files**:
   - Navigate to "Manage Files"
   - Select files to sync (they're automatically added)
   - Files are moved to the repo and symlinked automatically

4. **Sync with Remote**:
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

# Sync with custom commit message
dotstate sync -m "My custom commit message"

# Activate symlinks (useful after cloning on a new machine)
dotstate activate

# Deactivate symlinks (restore original files)
dotstate deactivate

# Check for updates and upgrade
dotstate upgrade

# Show help
dotstate help
```

## How It Works

1. **Storage**: Your dotfiles are stored in a Git repository (default: `~/.config/dotstate/storage`)
2. **Symlinks**: Original files are replaced with symlinks pointing to the repo
3. **Profiles**: Different profiles can have different sets of files
4. **Sync**: Changes are committed and synced with GitHub automatically

## Configuration

### Repository Setup Modes

DotState supports two repository setup modes:

#### GitHub Mode (Automatic)
Let DotState create and manage a GitHub repository for you. Requires a GitHub Personal Access Token.

**GitHub Token Configuration:**
1. **Environment Variable** (Recommended for automation):
   ```bash
   export DOTSTATE_GITHUB_TOKEN=ghp_your_token_here
   ```
   The environment variable takes precedence over the config file token.

2. **Config File**: The token can be stored in the config file (set during first-time setup).

#### Local Mode (Bring Your Own Repo)
Use any existing git repository from any host (GitHub, GitLab, Bitbucket, self-hosted, etc.).

**Setup:**
1. Create a repository on your preferred git host
2. Clone it locally: `git clone <url> ~/.config/dotstate/storage`
3. Ensure you can push: `git push origin main`
4. In DotState, choose "Use my own repository" and enter the path

**Benefits of Local Mode:**
- Works with any git host
- Uses your existing SSH keys or git credentials
- No GitHub token required

### Update Notifications

DotState automatically checks for updates and shows a notification in the main menu when a new version is available. You can also check manually:

```bash
# Check for updates interactively
dotstate upgrade

# Just check without prompting
dotstate upgrade --check
```

**Configuration:**
Update checks can be configured in `~/.config/dotstate/config.toml`:
```toml
[updates]
check_enabled = true       # Set to false to disable update checks
check_interval_hours = 24  # How often to check (default: 24 hours)
```

### Theme Configuration

DotState supports both light and dark themes that automatically adapt to your terminal background. The theme affects all UI elements including colors, borders, text, and syntax highlighting in file previews.

**Changing the Theme:**

Edit `~/.config/dotstate/config.toml` and set the `theme` option:

```toml
theme = "dark"   # For dark terminal backgrounds (default)
theme = "light"  # For light terminal backgrounds
theme = "nocolor" # Disable all UI colors (same as NO_COLOR=1 / --no-colors)
```

**Theme Features:**
- **Automatic Syntax Highlighting**: File preview syntax highlighting automatically matches your selected theme
- **Consistent Colors**: All UI elements (headers, footers, borders, lists, text) use theme-appropriate colors
- **Terminal Compatibility**: Works with both light and dark terminal themes
- **No Colors Mode**: Use `--no-colors` CLI flag or `NO_COLOR=1` to disable all colors:
  ```bash
  dotstate --no-colors
  ```
  Or:
  ```bash
  NO_COLOR=1 dotstate
  ```

**What Changes with Theme:**
- Text colors (dark text on light backgrounds, light text on dark backgrounds)
- Border colors (adjusted for visibility)
- Highlight colors (selection indicators, focused elements)
- Syntax highlighting themes in file previews
- Status colors (success, warning, error indicators)

### Keymap Configuration

DotState supports customizable keyboard shortcuts with preset keymaps (Standard, Vim, Emacs) and custom key binding overrides. The keymap system allows you to use your preferred keyboard layout and override any action with any key combination.

**Available Presets:**

- **Standard**: Arrow keys (↑↓), Enter, Esc, standard navigation
- **Vim**: Vim-style navigation (hjkl for movement, q to quit, etc.)
- **Emacs**: Emacs-style navigation (Ctrl+N/P for up/down, Ctrl+G to quit, etc.)

**Changing the Preset:**

Edit `~/.config/dotstate/config.toml` and set the `preset` option in the `[keymap]` section:

```toml
[keymap]
preset = "vim"  # Options: "standard", "vim", "emacs"
```

**Custom Key Binding Overrides:**

You can override any key binding with custom keys. Overrides take precedence over preset bindings and shadow preset bindings for the same action.

Example configuration:

```toml
[keymap]
preset = "vim"

# Override 'x' to quit instead of 'q'
[[keymap.overrides]]
key = "x"
action = "quit"

# Override 'w' to move up instead of 'k'
[[keymap.overrides]]
key = "w"
action = "move_up"

# Use Ctrl+H for help
[[keymap.overrides]]
key = "ctrl+h"
action = "help"
```

**Available Actions (all in snake_case):**

- **Navigation**: `move_up`, `move_down`, `move_left`, `move_right`, `page_up`, `page_down`, `go_to_top`, `go_to_end`, `home`, `end`
- **Selection**: `confirm`, `cancel`, `toggle_select`, `select_all`, `deselect_all`
- **Global**: `quit`, `help`
- **Actions**: `delete`, `edit`, `create`, `search`, `refresh`, `sync`, `check_status`, `install`
- **Text editing**: `backspace`, `delete_char`
- **Navigation**: `next_tab`, `prev_tab`
- **Scroll**: `scroll_up`, `scroll_down`
- **Prompts**: `yes`, `no`
- **Forms**: `save`, `toggle_backup`

**Key Format Examples:**

- Single keys: `"j"`, `"k"`, `"q"`, `"x"`
- Special keys: `"up"`, `"down"`, `"enter"`, `"esc"`, `"tab"`, `"space"`
- Function keys: `"f1"`, `"f2"`, etc.
- Modifier combinations: `"ctrl+n"`, `"ctrl+shift+j"`, `"ctrl+h"`

**How Overrides Work:**

- Overrides take precedence over preset bindings
- When you override an action (e.g., `move_up`), all preset bindings for that action are shadowed/removed
- If you override `move_up` with `"w"`, the original preset key (e.g., `"k"` in vim preset) will no longer work for that action
- Display functions (footer hints) automatically reflect your actual key bindings, including overrides

**Example:**

See `examples/keymap_override_example.toml` for a complete example configuration file.

## Security Considerations

- **No Shell Injection**: All commands use direct execution, not shell interpretation
- **Path Validation**: Dangerous paths (like home directory root) are blocked
- **Git Repository Detection**: Prevents nested Git repositories
- **Backup System**: Automatic backups before any file operation
- **Token Security**: GitHub tokens can be provided via `DOTSTATE_GITHUB_TOKEN` environment variable (recommended) or stored in config files with secure permissions

## Requirements

- **Rust**: Latest stable version (for building from source)
- **Git**: For repository operations
- **Git Account**: GitHub, GitLab, Bitbucket, or any git host (optional, for cloud sync)

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
