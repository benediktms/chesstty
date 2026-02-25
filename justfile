export CHESSTTY_DATA_DIR := "data"
export CHESSTTY_DB_PATH := "data/chesstty.db"
export CHESSTTY_SOCKET_PATH := "data/chesstty.sock"
export CHESSTTY_PID_PATH := "data/chesstty.pid"

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
    cargo run -p client-tui

# Run the shim CLI (starts server as daemon)
[group('app')]
start:
    cargo run -p chesstty

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

# Show what cargo-dist will build (validates release config)
[group('release')]
dist-plan:
    dist plan

# Build release artifacts for the current machine
[group('release')]
dist-build:
    dist build --artifacts=host --output-format=human

# Build release artifacts for a specific version tag
[group('release')]
dist-build-tag tag:
    dist build --tag={{tag}} --artifacts=host --output-format=human

# Check dynamic library linkage of release binaries
[group('release')]
dist-linkage:
    dist build --artifacts=host --print=linkage --output-format=human

# Regenerate the release CI workflow from dist-workspace.toml
[group('release')]
dist-generate:
    dist generate

# Preview changelog output (stdout only)
[group('release')]
changelog:
    git-cliff

# Update CHANGELOG.md in-place
[group('release')]
changelog-update:
    git-cliff -o CHANGELOG.md

# Bump version, update changelog, commit and tag (just tag patch/minor/major)
[group('release')]
tag level:
    ./scripts/tag-release.sh {{level}}
