#!/bin/bash
# DotState Installation Script
# This script installs DotState using Cargo if available, or downloads a pre-built binary

set -e

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

# Function to install via Cargo
install_via_cargo() {
    echo "📦 Installing DotState via Cargo..."
    cargo install dotstate

    if [ $? -eq 0 ]; then
        echo ""
        echo "✅ DotState installed successfully via Cargo!"
        echo ""
        echo "Run 'dotstate' to get started."
        exit 0
    else
        echo ""
        echo "⚠️  Cargo installation failed. Falling back to binary download..."
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
    # Standard Rust target triple naming
    if [ "$OS" = "linux" ]; then
        ASSET_NAME="dotstate-${ARCH}-unknown-linux-gnu${EXT}"
    elif [ "$OS" = "macos" ]; then
        if [ "$ARCH" = "arm64" ]; then
            ASSET_NAME="dotstate-aarch64-apple-darwin${EXT}"
        else
            ASSET_NAME="dotstate-x86_64-apple-darwin${EXT}"
        fi
    elif [ "$OS" = "windows" ]; then
        ASSET_NAME="dotstate-x86_64-pc-windows-msvc${EXT}"
    fi

    echo "📥 Downloading DotState binary for ${OS}-${ARCH}..."
    echo ""

    # Create install directory if it doesn't exist
    mkdir -p "$INSTALL_DIR"

    # Determine download URL
    if [ "$VERSION" = "latest" ]; then
        # Get latest release
        DOWNLOAD_URL=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | \
            grep "browser_download_url.*${ASSET_NAME}" | \
            cut -d '"' -f 4)
    else
        # Get specific version
        DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ASSET_NAME}"
    fi

    if [ -z "$DOWNLOAD_URL" ]; then
        echo "❌ Error: Could not find download URL for ${ASSET_NAME}"
        echo ""
        echo "Available releases: https://github.com/${REPO}/releases"
        exit 1
    fi

    echo "Downloading from: $DOWNLOAD_URL"

    # Download and install
    TEMP_FILE=$(mktemp)
    if curl -fsSL "$DOWNLOAD_URL" -o "$TEMP_FILE"; then
        mv "$TEMP_FILE" "$INSTALL_DIR/$BINARY_NAME${EXT}"
        chmod +x "$INSTALL_DIR/$BINARY_NAME${EXT}"

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
    else
        echo ""
        echo "❌ Error: Failed to download binary"
        echo ""
        echo "Please check:"
        echo "  1. Your internet connection"
        echo "  2. GitHub releases page: https://github.com/${REPO}/releases"
        exit 1
    fi
}

# Main installation logic
main() {
    # Try Cargo first if available
    if check_cargo; then
        echo "✅ Cargo detected. Attempting installation via Cargo..."
        echo ""
        if install_via_cargo; then
            exit 0
        fi
        # If Cargo installation fails, fall through to binary download
    else
        echo "ℹ️  Cargo not found. Will download pre-built binary instead."
        echo ""
    fi

    # Download binary
    download_binary
}

# Run main function
main
