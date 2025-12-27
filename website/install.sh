#!/bin/bash
# DotState Installation Script
# This script downloads a pre-built binary by default, falling back to Cargo if binaries fail

# Don't use set -e so we can handle errors gracefully and fall back to Cargo

VERSION="${DOTSTATE_VERSION:-latest}"
REPO="serkanyersen/dotstate"
INSTALL_DIR="$HOME/.local/bin"
BINARY_NAME="dotstate"

echo "🚀 Installing DotState..."
echo ""

# Function to detect OS and architecture
detect_system() {
    case "$(uname -s)" in
        Linux*)
            OS="linux"
            ;;
        Darwin*)
            OS="macos"
            ;;
        MINGW*|MSYS*|CYGWIN*)
            OS="windows"
            ;;
        *)
            echo "❌ Error: Unsupported operating system: $(uname -s)"
            exit 1
            ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)
            ARCH="x86_64"
            ;;
        arm64|aarch64)
            ARCH="arm64"
            ;;
        *)
            echo "❌ Error: Unsupported architecture: $(uname -m)"
            exit 1
            ;;
    esac
}

# Function to check if cargo is installed
check_cargo() {
    if command -v cargo &> /dev/null; then
        return 0
    else
        return 1
    fi
}

# Function to install via Cargo (fallback option)
install_via_cargo() {
    if ! command -v cargo &> /dev/null; then
        return 1
    fi

    echo "📦 Attempting installation via Cargo (fallback)..."
    echo ""
    if cargo install dotstate 2>/dev/null; then
        echo ""
        echo "✅ DotState installed successfully via Cargo!"
        echo ""
        echo "Run 'dotstate' to get started."
        exit 0
    else
        echo ""
        echo "❌ Cargo installation also failed."
        echo ""
        return 1
    fi
}

# Function to download binary from GitHub releases
download_binary() {
    detect_system

    # Determine file extension
    if [ "$OS" = "windows" ]; then
        EXT=".exe"
    else
        EXT=""
    fi

    # Determine asset name based on OS and architecture
    # Standard Rust target triple naming (matches GitHub release assets)
    if [ "$OS" = "linux" ]; then
        ASSET_NAME="dotstate-x86_64-unknown-linux-gnu.tar.gz"
    elif [ "$OS" = "macos" ]; then
        if [ "$ARCH" = "arm64" ]; then
            ASSET_NAME="dotstate-aarch64-apple-darwin.tar.gz"
        else
            ASSET_NAME="dotstate-x86_64-apple-darwin.tar.gz"
        fi
    elif [ "$OS" = "windows" ]; then
        ASSET_NAME="dotstate-x86_64-pc-windows-msvc.exe.tar.gz"
    fi

    echo "📥 Downloading DotState binary for ${OS}-${ARCH}..."
    echo ""

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Determine download URL
    if [ "$VERSION" = "latest" ]; then
        # Get latest release - match exact asset name (not .sha256 file)
        DOWNLOAD_URL=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep -o "\"browser_download_url\": \"[^\"]*${ASSET_NAME}\"" | \
            grep -v "\.sha256" | \
            cut -d '"' -f 4 | head -1)
    else
        # Get specific version
        DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_NAME}"
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        echo "❌ Error: Could not find download URL for ${ASSET_NAME}"
        echo ""
        echo "Available releases: https://github.com/${REPO}/releases"
        return 1
    fi

    echo "Downloading from: $DOWNLOAD_URL"

    # Download and extract
    TEMP_DIR=$(mktemp -d)
    TEMP_FILE="$TEMP_DIR/${ASSET_NAME}"

    if curl -fsSL "$DOWNLOAD_URL" -o "$TEMP_FILE"; then
        # Extract the archive
        cd "$TEMP_DIR"
        tar xzf "$TEMP_FILE"

        # Move binary to install directory
        if [ -f "$BINARY_NAME${EXT}" ]; then
            mv "$BINARY_NAME${EXT}" "$INSTALL_DIR/$BINARY_NAME${EXT}"
            chmod +x "$INSTALL_DIR/$BINARY_NAME${EXT}"
        else
            echo "❌ Error: Binary not found in archive"
            rm -rf "$TEMP_DIR"
            return 1
        fi

        # Cleanup
        rm -rf "$TEMP_DIR"

        echo ""
        echo "✅ DotState binary downloaded successfully!"
        echo ""

        # Check if binary is in PATH
        if echo "$PATH" | grep -q "$HOME/.local/bin"; then
            echo "🎉 Installation complete! Run 'dotstate' to get started."
        else
            echo "⚠️  Installation complete, but ~/.local/bin is not in your PATH."
            echo ""
            echo "Add this to your shell configuration file:"
            echo ""

            # Detect shell and provide appropriate instructions
            SHELL_NAME=$(basename "$SHELL")
            case "$SHELL_NAME" in
                bash)
                    CONFIG_FILE="$HOME/.bashrc"
                    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
                    echo "  source ~/.bashrc"
                    ;;
                zsh)
                    CONFIG_FILE="$HOME/.zshrc"
                    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.zshrc"
                    echo "  source ~/.zshrc"
                    ;;
                fish)
                    CONFIG_FILE="$HOME/.config/fish/config.fish"
                    echo "  echo 'set -gx PATH \$HOME/.local/bin \$PATH' >> ~/.config/fish/config.fish"
                    echo "  source ~/.config/fish/config.fish"
                    ;;
                *)
                    CONFIG_FILE="$HOME/.profile"
                    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.profile"
                    echo "  source ~/.profile"
                    ;;
            esac

            echo ""
            echo "Or run this command to add it temporarily:"
            echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
            echo ""
            echo "After adding to PATH, run 'dotstate' to get started."
        fi
        return 0
    else
        echo ""
        echo "❌ Error: Failed to download binary"
        echo ""
        return 1
    fi
}

# Main installation logic
main() {
    echo "📥 Attempting to download pre-built binary..."
    echo ""

    # Try binary download first (default method)
    if download_binary; then
        exit 0
    fi

    # If binary download fails, try Cargo as fallback
    echo "⚠️  Binary download failed. Trying Cargo as fallback..."
    echo ""

    if check_cargo; then
        if install_via_cargo; then
            exit 0
        fi
    else
        echo "❌ Cargo is not installed and binary download failed."
        echo ""
    fi

    # If both methods failed, show error
    echo "❌ Installation failed. Please check:"
    echo "  1. Your internet connection"
    echo "  2. GitHub releases page: https://github.com/${REPO}/releases"
    echo "  3. Install Rust/Cargo if you want to build from source: https://rustup.rs/"
    exit 1
}

# Run main function
main
