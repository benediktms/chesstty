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
//!   daemonized via double-fork + exec whose PID is recorded in a PID file.
//! - **client-tui** (or `cargo run -p client-tui`): the terminal UI, run in the
//!   foreground; the shim exits when the TUI exits.
//! - **chesstty** (this binary): the supervisor shim that wires the two together.
//!
//! Communication between the shim and the server uses a Unix domain socket whose
//! path is controlled by the `CHESSTTY_SOCKET_PATH` environment variable (see
//! [`config`] for all tunables).
//!
//! # Fork safety
//!
//! The `main()` function is intentionally **sync** (no `#[tokio::main]`).
//! All fork/daemon logic runs before any tokio runtime is created, because
//! forking a multi-threaded process is undefined behavior. The tokio runtime
//! is created manually *after* the fork boundary, only in the parent path.

use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use clap::{Parser, Subcommand};

mod config;
mod daemon;
mod process;
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

/// Spawn the chess engine server as a daemonized background process.
///
/// Uses the classic fork + [`daemon::Daemon`] (double-fork) + exec pattern:
///
/// 1. Clean up stale PID file and socket.
/// 2. `fork()` — the child daemonizes, the parent waits for the child to exit
///    and then returns.
/// 3. **Child path**: calls [`daemon::Daemon::start`] which performs a
///    double-fork, `setsid`, IO redirection, and PID file write. The resulting
///    daemon process then `exec`s into the `chesstty-server` binary (with a
///    `cargo run` fallback for development).
/// 4. **Parent path**: `waitpid` on the direct child (which exits quickly after
///    the daemon's first fork) and returns to the caller.
///
/// # Safety
///
/// The `fork()` and `waitpid()` calls require that no tokio runtime (or any
/// multi-threaded runtime) exists at call time. The caller (`main`) is sync
/// and single-threaded, satisfying this invariant.
///
/// # Errors
///
/// Returns [`CliError::ProcessError`] if `fork()` fails.
fn spawn_server() -> Result<(), CliError> {
    let pid_path = config::get_pid_path();
    let log_path = config::get_server_log_path();

    // Clean up stale state
    let _ = process::remove_stale_pid(&pid_path);
    let socket_path = config::get_socket_path();
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }

    // Fork — child becomes daemon, parent continues.
    // SAFETY: No tokio runtime exists yet (main is sync). Single-threaded.
    let child_pid = unsafe { libc::fork() };
    match child_pid {
        -1 => Err(CliError::ProcessError(format!(
            "fork failed: {}",
            std::io::Error::last_os_error()
        ))),
        0 => {
            // === CHILD PATH ===
            // Daemonize via builder (double-fork, setsid, IO redirect, PID write).
            let stdout =
                File::create(&log_path).unwrap_or_else(|_| File::open("/dev/null").unwrap());
            let stderr = stdout
                .try_clone()
                .unwrap_or_else(|_| File::open("/dev/null").unwrap());

            if let Err(e) = daemon::Daemon::new()
                .pid_file(&pid_path)
                .working_directory("/tmp")
                .umask(0o027)
                .stdout(stdout)
                .stderr(stderr)
                .start()
            {
                eprintln!("Daemon start failed: {}", e);
                std::process::exit(1);
            }

            // We are now the daemon (grandchild). Exec into server binary.
            use std::os::unix::process::CommandExt;
            let server_bin = resolve_sibling_binary("chesstty-server");
            let err = Command::new(&server_bin).exec();

            // exec() only returns on failure — try cargo fallback
            if err.kind() == std::io::ErrorKind::NotFound {
                let err = Command::new("cargo")
                    .args(["run", "-p", "chesstty-server"])
                    .exec();
                eprintln!("Failed to exec server (cargo fallback): {}", err);
            } else {
                eprintln!("Failed to exec server binary: {}", err);
            }
            std::process::exit(1);
        }
        _ => {
            // === PARENT PATH (shim) ===
            // Wait for the first child to exit (it exits quickly after Daemon fork).
            // SAFETY: waitpid on our direct child is safe.
            let mut status: libc::c_int = 0;
            unsafe { libc::waitpid(child_pid, &mut status, 0) };
            // Parent continues — the daemon is running independently.
            Ok(())
        }
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
        Err(e) => {
            return Err(CliError::ProcessError(format!(
                "failed to spawn TUI: {}",
                e
            )))
        }
    };

    // Wait for the client to finish
    let status = child
        .wait()
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
    if !process::is_server_running(&pid_path) {
        println!("Server is not running.");
        return Ok(());
    }

    // Read the PID and send the appropriate signal
    let pid = process::read_pid(&pid_path)
        .map_err(|e| CliError::ProcessError(format!("failed to read PID: {}", e)))?;

    let signal = if force { libc::SIGKILL } else { libc::SIGTERM };

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

    println!(
        "Server stopped (signal: {}).",
        if force { "SIGKILL" } else { "SIGTERM" }
    );

    Ok(())
}

/// Entry point for the ChessTTY shim.
///
/// This function is intentionally **sync** — no `#[tokio::main]`. All
/// fork/daemon logic in [`spawn_server`] must execute before any multi-threaded
/// runtime exists (forking a tokio runtime is undefined behavior). The tokio
/// runtime is created manually after the fork boundary, solely for the async
/// socket-readiness wait.
///
/// Overall flow when no subcommand is given:
/// 1. Parse the command line with [`Cli`].
/// 2. If the server is not already running, call [`spawn_server`] (fork + daemon + exec).
/// 3. Create a tokio runtime and wait for the server's Unix socket ([`wait_for_server_socket`]).
/// 4. Launch the TUI with [`spawn_tui_client`] and block until it exits.
///
/// When the `engine stop` subcommand is given, delegates directly to
/// [`handle_engine_stop`] — no runtime needed.
///
/// # Errors
///
/// Propagates any [`CliError`] returned by the steps above, causing the process
/// to exit with a non-zero status code.
fn main() -> Result<(), CliError> {
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
            let server_running = process::is_server_running(&pid_path);

            if !server_running {
                tracing::info!("Server not running, starting...");
                spawn_server()?;
                tracing::info!("Server spawned.");
            } else {
                tracing::info!("Server already running.");
            }

            // Create tokio runtime AFTER fork boundary.
            // SAFETY invariant: no runtime existed during spawn_server().
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| CliError::ProcessError(format!("failed to create runtime: {}", e)))?;

            // Wait for socket to be ready
            tracing::info!("Waiting for server socket...");
            rt.block_on(wait_for_server_socket())?;
            tracing::info!("Server socket ready.");

            // Spawn TUI client (sync — doesn't need tokio)
            tracing::info!("Starting TUI client...");
            spawn_tui_client()?;

            tracing::info!("TUI exited.");
        }
    }

    Ok(())
}
