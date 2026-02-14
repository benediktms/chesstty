#!/usr/bin/env bash
set -euo pipefail

echo "=== Stockfish Installation Script ==="
echo

# Check if stockfish is already installed
if command -v stockfish &>/dev/null; then
    echo "Stockfish is already installed: $(which stockfish)"
    stockfish --help 2>/dev/null | head -1 || true
    echo "Nothing to do."
    exit 0
fi

echo "Stockfish not found. Attempting to install..."
echo

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin)
        echo "Detected macOS"
        if command -v brew &>/dev/null; then
            echo "Installing via Homebrew..."
            brew install stockfish
        else
            echo "Error: Homebrew not found."
            echo "Install Homebrew first: https://brew.sh"
            echo "Then run: brew install stockfish"
            exit 1
        fi
        ;;
    Linux)
        echo "Detected Linux"
        if [ -f /etc/os-release ]; then
            . /etc/os-release
            case "$ID" in
                ubuntu|debian|pop|linuxmint)
                    echo "Installing via apt (${PRETTY_NAME})..."
                    sudo apt-get update -qq
                    sudo apt-get install -y stockfish
                    ;;
                fedora|rhel|centos|rocky|alma)
                    echo "Installing via dnf (${PRETTY_NAME})..."
                    sudo dnf install -y stockfish
                    ;;
                arch|manjaro|endeavouros)
                    echo "Installing via pacman (${PRETTY_NAME})..."
                    sudo pacman -S --noconfirm stockfish
                    ;;
                *)
                    echo "Unknown Linux distribution: $ID"
                    echo "Please install Stockfish manually:"
                    echo "  https://stockfishchess.org/download/"
                    exit 1
                    ;;
            esac
        else
            echo "Cannot detect Linux distribution."
            echo "Please install Stockfish manually:"
            echo "  https://stockfishchess.org/download/"
            exit 1
        fi
        ;;
    *)
        echo "Unsupported OS: $OS"
        echo "Please install Stockfish manually:"
        echo "  https://stockfishchess.org/download/"
        exit 1
        ;;
esac

echo
# Verify installation
if command -v stockfish &>/dev/null; then
    echo "Stockfish installed successfully: $(which stockfish)"
else
    echo "Error: Stockfish installation may have failed."
    echo "Please check the output above and try installing manually."
    exit 1
fi
