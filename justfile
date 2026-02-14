# Default recipe - show all available commands
default:
    @just --list

# Run the server
[group('app')]
server:
    cargo run -p chesstty-server

# Run the TUI client
[group('app')]
tui:
    cargo run -p chesstty-tui

# Run all tests
[group('test')]
test scope="--workspace" *opt:
    cargo test {{scope}} {{opt}}

# Build all crates
[group('build')]
build:
    cargo build --workspace

# Build release
[group('build')]
release:
    cargo build --workspace --release

# Clean build artifacts
[group('build')]
clean:
    cargo clean

# Install stockfish (if not already installed)
[group('checks')]
stockfish:
    ./scripts/install-stockfish.sh

# Run clippy lints
[group('checks')]
lint:
    cargo clippy --workspace -- -W warnings
