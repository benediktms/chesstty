# chesstty

The CLI shim and process supervisor for ChessTTY. A single entry point that replaces the two-terminal workflow with one unified command.

## Overview

The chesstty binary acts as a process supervisor and orchestrator. It manages the lifecycle of the gRPC server backend and TUI client, replacing the need for manual `just server` + `just tui` commands with a single `chesstty` invocation.

### Workflow

```
chesstty (no args)
  ├── Check if server is running (PID file + kill(pid, 0))
  ├── If not running: spawn chesstty-server as background process
  │   └── Fallback: if binary not found, use `cargo run -p chesstty-server`
  ├── Wait for UDS socket to become ready (poll with timeout)
  ├── Spawn client-tui in foreground
  │   └── Fallback: if binary not found, use `cargo run -p client-tui`
  └── Wait for TUI to exit, propagate exit status

chesstty engine stop [--force]
  ├── Read PID from PID file
  ├── Send SIGTERM (or SIGKILL with --force)
  └── Remove PID file
```

## Usage

### Start ChessTTY

```bash
chesstty
```

Launches the server (if not already running) and opens the terminal UI.

### Stop the Server

```bash
chesstty engine stop
```

Gracefully shuts down the background server process via SIGTERM.

```bash
chesstty engine stop --force
```

Immediately kills the server via SIGKILL (for stuck processes).

## Architecture

The shim coordinates three components:

- **chesstty-server**: The gRPC analysis engine, spawned as a detached background process with its PID recorded in a PID file
- **client-tui**: The terminal UI, run in the foreground; the shim exits when the TUI exits
- **chesstty** (this binary): The supervisor that wires them together

### Server Discovery

The shim uses a PID file to track whether the server is running:

1. Read the PID from the file (if it exists)
2. Use `kill(pid, 0)` to check if the process exists
3. If not running, spawn a new server and write its PID to the file
4. If stale PID files are found (process no longer exists), remove them before spawning

### Binary Resolution

Both the server and TUI are discovered using a fallback chain:

1. Check for a sibling binary in the same directory as the chesstty executable
2. If not found, fall back to `cargo run -p <crate>` (for development)

This allows the shim to work both with compiled binaries and in development environments.

### Socket Readiness Polling

After spawning the server, the shim waits for the gRPC server to become ready by polling for a Unix domain socket connection:

1. Check if the socket file exists
2. Try to connect to the socket
3. If either step fails, sleep briefly and retry
4. Time out after the configured duration

This ensures the TUI doesn't launch until the server is accepting connections.

## Modules

### main.rs

Entry point. Parses CLI arguments via clap, coordinates the startup sequence, and handles the `engine stop` subcommand.

Key functions:
- `spawn_server()` - Spawns the backend server process as a daemon, handles fallback to `cargo run`
- `wait_for_server_socket()` - Async function that polls for socket availability
- `spawn_tui_client()` - Spawns the TUI in the foreground and waits for it to exit
- `handle_engine_stop()` - Sends SIGTERM/SIGKILL to the server process

### config.rs

Runtime configuration via environment variables. All values have sensible defaults.

Configuration functions:
- `get_socket_path()` - Path to the UDS socket (default: `/tmp/chesstty.sock`)
- `get_pid_path()` - Path to the PID file (default: `/tmp/chesstty.pid`)
- `get_socket_timeout_secs()` - Socket readiness timeout in seconds (default: 5)
- `get_socket_poll_interval_ms()` - Polling interval in milliseconds (fixed: 100)
- `get_server_log_path()` - Server stdout/stderr log file (default: `/dev/null`)

### daemon.rs

Process management utilities for detecting and controlling the server.

Key functions:
- `read_pid(pid_path)` - Reads a PID from a file
- `is_server_running(pid_path)` - Checks if the process is alive via `kill(pid, 0)`
- `remove_stale_pid(pid_path)` - Removes PID files for processes that no longer exist

### wait.rs

Async socket polling to wait for the server to become connectable.

Key functions:
- `wait_for_socket(socket_path, timeout, poll_interval)` - Polls for socket availability with timeout
- `wait_for_socket_default(socket_path)` - Convenience function with default settings (5s timeout, 100ms poll)

## Configuration

All configuration is via environment variables with sensible defaults:

| Variable | Purpose | Default |
|----------|---------|---------|
| `CHESSTTY_SOCKET_PATH` | UDS socket path for client-server communication | `/tmp/chesstty.sock` |
| `CHESSTTY_PID_PATH` | Path to the server's PID file | `/tmp/chesstty.pid` |
| `CHESSTTY_SOCKET_TIMEOUT_SECS` | Socket readiness timeout in seconds | `5` |
| `CHESSTTY_SERVER_LOG_PATH` | Server process stdout/stderr log file | `/dev/null` |

### Example: Custom Socket Path

```bash
CHESSTTY_SOCKET_PATH=/var/run/chesstty.sock \
CHESSTTY_PID_PATH=/var/run/chesstty.pid \
chesstty
```

### Example: Enable Server Logging

```bash
CHESSTTY_SERVER_LOG_PATH=$HOME/.local/share/chesstty/server.log \
chesstty
```

## How It Works

### Startup Sequence

1. Parse CLI arguments (only `engine stop` or default startup)
2. Get configuration from environment variables
3. Check if server is running by reading PID file and calling `kill(pid, 0)`
4. If not running:
   - Remove any stale PID files
   - Try to spawn `chesstty-server` binary (or `cargo run -p chesstty-server` as fallback)
   - Write the server's PID to the PID file
5. Wait for the server's UDS socket to become available (with timeout)
6. Spawn `client-tui` in the foreground (or `cargo run -p client-tui` as fallback)
7. Wait for the TUI to exit and propagate its exit status

### Shutdown Sequence (engine stop)

1. Check if server is running (read PID and verify with `kill(pid, 0)`)
2. If running, send SIGTERM (or SIGKILL with `--force`)
3. Remove the PID file
4. Print confirmation message

### Error Handling

All errors in the startup sequence are typed via `CliError`:
- `DaemonStart` - Failed to spawn or control the server
- `SocketWait` - Timeout waiting for socket, or connection errors
- `ClientSpawn` - Failed to spawn the TUI
- `ProcessError` - Other process-related failures (PID file I/O, signal delivery)

## Dependencies

- **clap** - CLI argument parsing with derive macros
- **libc** - UNIX signal operations (kill, fork, dup2) for process control
- **tokio** - Async runtime for socket polling
- **thiserror/anyhow** - Error handling
- **tracing/tracing-subscriber** - Structured logging

## Testing

```bash
cargo test -p chesstty
```

Tests are co-located with source code:

- **main.rs** - Log file creation and server stdio setup
- **config.rs** - Environment variable parsing and defaults
- **daemon.rs** - PID file operations, process existence checks
- **wait.rs** - Socket polling with timeout and reconnection logic

## Logging

The shim logs startup and shutdown events to stderr via the tracing framework. By default, logging is at INFO level. Set `RUST_LOG=debug` for more detailed output:

```bash
RUST_LOG=debug chesstty
```

Output includes:
- Server startup status
- Socket path and PID file paths
- Socket readiness waiting
- TUI client launch and exit
