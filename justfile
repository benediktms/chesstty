# Default recipe - show all available commands
default:
    @just --list

# Run the server
server:
    cargo run -p chesstty-server

# Run the TUI client
tui:
    cargo run -p chesstty-tui

# Run server and TUI concurrently (server in background, cleaned up on exit)
play:
    #!/usr/bin/env bash
    set -e
    echo "Starting server..."
    cargo run -p chesstty-server &
    SERVER_PID=$!
    trap "kill $SERVER_PID 2>/dev/null; wait $SERVER_PID 2>/dev/null" EXIT
    sleep 1
    echo "Starting TUI..."
    cargo run -p chesstty-tui
    echo "TUI exited, stopping server..."

# Run all tests
test:
    cargo test --workspace

# Build all crates
build:
    cargo build --workspace

# Build release
release:
    cargo build --workspace --release

# Install stockfish (if not already installed)
stockfish:
    ./scripts/install-stockfish.sh

# Check if stockfish is installed
check-stockfish:
    @which stockfish 2>/dev/null && echo "Stockfish found: $(which stockfish)" || echo "Stockfish not found. Run: just stockfish"

# Clean build artifacts
clean:
    cargo clean

# Run clippy lints
lint:
    cargo clippy --workspace -- -W warnings
