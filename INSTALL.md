# Installation Guide

## Quick Install

### Using Cargo (Recommended)

```bash
cargo install dotstate
```

### From Source

```bash
# Clone the repository
git clone https://github.com/serkanyersen/dotstate.git
cd dotstate

# Build and install
cargo install --path .
```

### Using Homebrew (macOS)

```bash
# Add the tap (once)
brew tap serkanyersen/dotstate

# Install
brew install dotstate
```

Or use the direct install:
```bash
brew install serkanyersen/dotstate/dotstate
```

## Building from Source

### Prerequisites

- **Rust**: Latest stable version (1.70+)
- **Cargo**: Comes with Rust
- **Git**: For cloning the repository

### Steps

1. **Clone the repository**:
   ```bash
   git clone https://github.com/serkanyersen/dotstate.git
   cd dotstate
   ```

2. **Build in release mode**:
   ```bash
   cargo build --release
   ```

3. **Install globally** (optional):
   ```bash
   cargo install --path .
   ```

   This will install `dotstate` to `~/.cargo/bin/` (make sure it's in your PATH).

4. **Or run directly**:
   ```bash
   ./target/release/dotstate
   ```

## Verifying Installation

After installation, verify it works:

```bash
dotstate --version
```

You should see the version number.

## Troubleshooting

### Command not found

If `dotstate` is not found after installation:

1. **Check if Cargo bin directory is in PATH**:
   ```bash
   echo $PATH | grep cargo
   ```

2. **Add to PATH** (add to your `~/.zshrc` or `~/.bashrc`):
   ```bash
   export PATH="$HOME/.cargo/bin:$PATH"
   ```

3. **Reload your shell**:
   ```bash
   source ~/.zshrc  # or ~/.bashrc
   ```

### Build Errors

If you encounter build errors:

1. **Update Rust**:
   ```bash
   rustup update stable
   ```

2. **Check Rust version**:
   ```bash
   rustc --version
   ```

   Should be 1.70 or later.

3. **Clean and rebuild**:
   ```bash
   cargo clean
   cargo build --release
   ```

## Platform-Specific Notes

### macOS

- Works out of the box
- No additional dependencies required

### Linux

- May need to install development tools:
  ```bash
  # Ubuntu/Debian
  sudo apt-get install build-essential pkg-config libssl-dev

  # Fedora
  sudo dnf install gcc openssl-devel
  ```

### Windows

- Requires Visual Studio Build Tools or MinGW
- Git for Windows is recommended
- Terminal should support ANSI escape codes (Windows Terminal recommended)

## Next Steps

After installation, see the [README.md](README.md) for usage instructions.

### First-time Setup

When you first run `dotstate`, you'll be asked to choose a setup mode:

1. **Create for me (GitHub)**: DotState creates a repository on your GitHub account
   - Requires a GitHub Personal Access Token
   - Best for simple, automated setup

2. **Use my own repository**: Bring your own git repository
   - Works with GitHub, GitLab, Bitbucket, or any git host
   - Uses your existing SSH keys or git credentials
   - No token required

