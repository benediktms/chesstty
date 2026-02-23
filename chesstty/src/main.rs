//! ChessTTY CLI shim - Process supervisor and entry point.
//!
//! This binary acts as the single entry point for ChessTTY, replacing the
//! current two-terminal workflow (`just server` + `just tui`) with a single
//! command that:
//!
//! 1. **Default mode (no args)**: Launches the gRPC server as a background
//!    daemon (if not already running) and then launches the TUI in the foreground.
//! 2. **`engine stop` subcommand**: Signals the background server to shut down
//!    gracefully (SIGTERM) or immediately (`--force` → SIGKILL).
//!
//! # Architecture
//!
//! The shim coordinates three processes:
//! - **chesstty-server** (or `cargo run -p chesstty-server`): the gRPC analysis engine,
//!   spawned as a detached background process whose PID is recorded in a PID file.
//! - **client-tui** (or `cargo run -p client-tui`): the terminal UI, run in the
//!   foreground; the shim exits when the TUI exits.
//! - **chesstty** (this binary): the supervisor shim that wires the two together.
//!
//! Communication between the shim and the server uses a Unix domain socket whose
//! path is controlled by the `CHESSTTY_SOCKET_PATH` environment variable (see
//! [`config`] for all tunables).


use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};

mod config;
mod daemon;
mod wait;

/// Top-level CLI arguments for ChessTTY.
///
/// When invoked with no subcommand, the shim starts the server (if needed) and
/// launches the TUI. Subcommands provide management operations.
#[derive(Parser)]
#[command(name = "chesstty", about = "Chess TUI with integrated engine analysis")]
struct Cli {
    /// Optional subcommand. When omitted, runs the default server + TUI flow.
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Top-level subcommands for managing ChessTTY components.
#[derive(Subcommand)]
enum Commands {
    /// Manage the background chess engine server.
    Engine {
        /// The engine management action to perform.
        #[command(subcommand)]
        action: EngineAction,
    },
}

/// Actions that can be performed on the background engine server.
#[derive(Subcommand)]
enum EngineAction {
    /// Stop the running engine server.
    ///
    /// Sends SIGTERM by default for a graceful shutdown. Pass `--force` to
    /// send SIGKILL for an immediate termination.
    Stop {
        /// Send SIGKILL instead of SIGTERM for an immediate (non-graceful) stop.
        #[arg(short, long)]
        force: bool,
    },
}

/// Error type for CLI operations.
#[derive(Debug, thiserror::Error)]
enum CliError {
    /// The server daemon failed to start (double-fork or privilege-drop error).
    #[error("failed to start server daemon: {0}")]
    DaemonStart(#[from] daemon::DaemonError),

    /// Timed out or failed to connect while waiting for the server socket.
    #[error("failed to wait for socket: {0}")]
    SocketWait(#[from] wait::WaitError),

    /// The TUI client process could not be spawned.
    #[error("failed to spawn client: {0}")]
    ClientSpawn(#[from] std::io::Error),

    /// A general process-management error (spawn failure, PID I/O, signal delivery).
    #[error("server process error: {0}")]
    ProcessError(String),
}

/// Resolve the path to a sibling binary distributed alongside this executable.
///
/// The resolution strategy is:
/// 1. Inspect the directory that contains the currently running executable.
/// 2. If `<dir>/<name>` exists on disk, return that path.
/// 3. Otherwise fall back to returning `name` as-is, relying on `PATH` lookup.
///
/// This allows the installed release layout (`chesstty`, `chesstty-server`, and
/// `client-tui` in the same directory) to work without any additional
/// configuration, while still allowing `cargo run` development workflows where
/// the binaries are not co-located.
fn resolve_sibling_binary(name: &str) -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(name);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    PathBuf::from(name)
}

/// Build the `(stdout, stderr)` `Stdio` pair that the server process should inherit.
///
/// If `log_path` is `/dev/null`, both streams are discarded via [`Stdio::null`].
/// Otherwise the file at `log_path` is opened (or created) in append mode and
/// both stdout and stderr are directed to it, so all server output lands in a
/// single log file.
///
/// # Errors
///
/// Returns [`CliError::ProcessError`] if the log file cannot be opened or cloned.
fn server_log_stdio_for_path(log_path: &std::path::Path) -> Result<(Stdio, Stdio), CliError> {
    if log_path == std::path::Path::new("/dev/null") {
        return Ok((Stdio::null(), Stdio::null()));
    }

    let stdout_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| CliError::ProcessError(format!("failed to open server log file: {}", e)))?;

    let stderr_file = stdout_file
        .try_clone()
        .map_err(|e| CliError::ProcessError(format!("failed to clone server log file: {}", e)))?;

    Ok((Stdio::from(stdout_file), Stdio::from(stderr_file)))
}

/// Build the `(stdout, stderr)` `Stdio` pair using the configured server log path.
///
/// Reads the log path from [`config::get_server_log_path`] (which honours the
/// `CHESSTTY_SERVER_LOG_PATH` environment variable) and delegates to
/// [`server_log_stdio_for_path`].
///
/// # Errors
///
/// Propagates any error from [`server_log_stdio_for_path`].
fn server_log_stdio() -> Result<(Stdio, Stdio), CliError> {
    let log_path = config::get_server_log_path();
    server_log_stdio_for_path(&log_path)
}

/// Spawn the chess engine server as a detached background process.
///
/// Resolution strategy (tried in order):
/// 1. Look for a `chesstty-server` binary next to the current executable via
///    [`resolve_sibling_binary`].
/// 2. If that binary is not found (`NotFound` I/O error), fall back to
///    `cargo run -p chesstty-server` for development workflows.
///
/// The server's stdout and stderr are redirected to the configured log file
/// (see [`server_log_stdio`]). The spawned child's PID is recorded in the PID
/// file returned by [`config::get_pid_path`] so that [`handle_engine_stop`] can
/// signal it later.
///
/// # Errors
///
/// Returns [`CliError::ProcessError`] if the server binary (and the cargo
/// fallback) both fail to spawn, or if the PID file cannot be written.
fn spawn_server() -> Result<(), CliError> {
    let pid_path = config::get_pid_path();

    // Remove stale PID file if it exists
    let _ = daemon::remove_stale_pid(&pid_path);

    let server_bin = resolve_sibling_binary("chesstty-server");
    let (stdout_stdio, stderr_stdio) = server_log_stdio()?;
    let child = match Command::new(&server_bin)
        .stdin(Stdio::null())
        .stdout(stdout_stdio)
        .stderr(stderr_stdio)
        .spawn()
    {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            let (fallback_stdout, fallback_stderr) = server_log_stdio()?;
            Command::new("cargo")
                .args(["run", "-p", "chesstty-server"])
                .stdin(Stdio::null())
                .stdout(fallback_stdout)
                .stderr(fallback_stderr)
                .spawn()
                .map_err(|spawn_err| {
                    CliError::ProcessError(format!(
                        "failed to spawn server (binary: {}, cargo fallback: {})",
                        e, spawn_err
                    ))
                })?
        }
        Err(e) => {
            return Err(CliError::ProcessError(format!(
                "failed to spawn server binary: {}",
                e
            )))
        }
    };

    // Write the PID to the PID file
    let pid = child.id();
    std::fs::write(&pid_path, format!("{}\n", pid))
        .map_err(|e| CliError::ProcessError(format!("failed to write PID file: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::server_log_stdio_for_path;

    #[test]
    fn test_server_log_stdio_creates_log_file_for_path_sink() {
        let tempdir = tempfile::tempdir().expect("failed to create temp dir");
        let log_path = tempdir.path().join("server.log");

        assert!(!log_path.exists());
        let _ = server_log_stdio_for_path(&log_path).expect("failed to create log stdio");
        assert!(log_path.exists());
    }
}

/// Wait for the server's Unix domain socket to become connectable.
///
/// Polls the socket path (from [`config::get_socket_path`]) at the configured
/// interval until either a connection succeeds or the timeout elapses. Both the
/// timeout and poll interval are read from [`config`] and can be overridden via
/// environment variables.
///
/// # Errors
///
/// Returns [`CliError::SocketWait`] wrapping a [`wait::WaitError`] if the socket
/// does not become available within the configured timeout.
async fn wait_for_server_socket() -> Result<(), CliError> {
    let socket_path = config::get_socket_path();
    let timeout = Duration::from_secs(config::get_socket_timeout_secs());
    let poll_interval = Duration::from_millis(config::get_socket_poll_interval_ms());

    wait::wait_for_socket(&socket_path, timeout, poll_interval)
        .await
        .map_err(CliError::from)
}

/// Spawn the TUI client and block until it exits.
///
/// Resolution strategy mirrors [`spawn_server`]:
/// 1. Try a `client-tui` binary co-located with the current executable.
/// 2. Fall back to `cargo run -p client-tui` if the binary is not found.
///
/// The shim runs in the foreground while the TUI is active. When the TUI exits,
/// its exit status is propagated: a non-zero status causes this function to
/// return [`CliError::ProcessError`] so the shim exits with a non-zero code.
///
/// # Errors
///
/// Returns [`CliError::ProcessError`] if the TUI cannot be spawned, if waiting
/// for it fails, or if it exits with a non-zero status.
fn spawn_tui_client() -> Result<(), CliError> {
    let client_bin = resolve_sibling_binary("client-tui");
    let mut child = match Command::new(&client_bin).spawn() {
        Ok(child) => child,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Command::new("cargo")
            .args(["run", "-p", "client-tui"])
            .spawn()
            .map_err(|spawn_err| {
                CliError::ProcessError(format!(
                    "failed to spawn TUI (binary: {}, cargo fallback: {})",
                    e, spawn_err
                ))
            })?,
        Err(e) => return Err(CliError::ProcessError(format!("failed to spawn TUI: {}", e))),
    };

    // Wait for the client to finish
    let status = child.wait()
        .map_err(|e| CliError::ProcessError(format!("failed to wait for TUI: {}", e)))?;

    // If the client exited with an error, propagate it
    if !status.success() {
        return Err(CliError::ProcessError(format!(
            "TUI client exited with status: {}",
            status
        )));
    }

    Ok(())
}

/// Send a stop signal to the running engine server and remove its PID file.
///
/// Reads the PID from the configured PID file and sends:
/// - `SIGTERM` when `force` is `false` — requests a graceful shutdown.
/// - `SIGKILL` when `force` is `true` — terminates immediately.
///
/// If no PID file is found or the recorded process is not running, the function
/// prints a message and returns `Ok(())` without error. On success it removes
/// the PID file so the next `chesstty` invocation knows the server is gone.
///
/// # Errors
///
/// Returns [`CliError::ProcessError`] if the PID file cannot be read or if the
/// `kill(2)` system call fails.
fn handle_engine_stop(force: bool) -> Result<(), CliError> {
    let pid_path = config::get_pid_path();

    // Check if server is running
    if !daemon::is_server_running(&pid_path) {
        println!("Server is not running.");
        return Ok(());
    }

    // Read the PID and send the appropriate signal
    let pid = daemon::read_pid(&pid_path)
        .map_err(|e| CliError::ProcessError(format!("failed to read PID: {}", e)))?;

    let signal = if force {
        libc::SIGKILL
    } else {
        libc::SIGTERM
    };

    // SAFETY: kill is safe when sending signals to our own spawned processes
    let result = unsafe { libc::kill(pid, signal) };

    if result != 0 {
        return Err(CliError::ProcessError(format!(
            "failed to send signal to process {}: {}",
            pid,
            std::io::Error::last_os_error()
        )));
    }

    // Remove the PID file
    let _ = std::fs::remove_file(&pid_path);

    println!("Server stopped (signal: {}).", if force { "SIGKILL" } else { "SIGTERM" });

    Ok(())
}

/// Entry point for the ChessTTY shim.
///
/// Overall flow when no subcommand is given:
/// 1. Parse the command line with [`Cli`].
/// 2. If the server is not already running, call [`spawn_server`].
/// 3. Wait for the server's Unix socket to be connectable ([`wait_for_server_socket`]).
/// 4. Launch the TUI with [`spawn_tui_client`] and block until it exits.
///
/// When the `engine stop` subcommand is given, delegates directly to
/// [`handle_engine_stop`].
///
/// # Errors
///
/// Propagates any [`CliError`] returned by the steps above, causing the process
/// to exit with a non-zero status code.
#[tokio::main]
async fn main() -> Result<(), CliError> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Engine { action }) => match action {
            EngineAction::Stop { force } => {
                handle_engine_stop(force)?;
            }
        },
        None => {
            let socket_path = config::get_socket_path();
            let pid_path = config::get_pid_path();

            tracing::info!("Starting ChessTTY...");
            tracing::debug!("Socket: {:?}", socket_path);
            tracing::debug!("PID file: {:?}", pid_path);

            // Check if server is already running
            let server_running = daemon::is_server_running(&pid_path);

            if !server_running {
                tracing::info!("Server not running, starting...");
                spawn_server()?;
                tracing::info!("Server spawned.");
            } else {
                tracing::info!("Server already running.");
            }

            // Wait for socket to be ready
            tracing::info!("Waiting for server socket...");
            wait_for_server_socket().await?;
            tracing::info!("Server socket ready.");

            // Spawn TUI client
            tracing::info!("Starting TUI client...");
            spawn_tui_client()?;

            tracing::info!("TUI exited.");
        }
    }

    Ok(())
}
