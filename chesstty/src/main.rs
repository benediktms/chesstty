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


use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};

mod config;
mod daemon;
mod wait;

#[derive(Parser)]
#[command(name = "chesstty", about = "Chess TUI with integrated engine analysis")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Engine {
        #[command(subcommand)]
        action: EngineAction,
    },
}

#[derive(Subcommand)]
enum EngineAction {
    Stop {
        #[arg(short, long)]
        force: bool,
    },
}

/// Error type for CLI operations.
#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("failed to start server daemon: {0}")]
    DaemonStart(#[from] daemon::DaemonError),

    #[error("failed to wait for socket: {0}")]
    SocketWait(#[from] wait::WaitError),

    #[error("failed to spawn client: {0}")]
    ClientSpawn(#[from] std::io::Error),

    #[error("server process error: {0}")]
    ProcessError(String),
}

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

fn server_log_stdio() -> Result<(Stdio, Stdio), CliError> {
    let log_path = config::get_server_log_path();
    server_log_stdio_for_path(&log_path)
}

/// Start the server as a background process.
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

/// Wait for the server socket to become available.
async fn wait_for_server_socket() -> Result<(), CliError> {
    let socket_path = config::get_socket_path();
    let timeout = Duration::from_secs(config::get_socket_timeout_secs());
    let poll_interval = Duration::from_millis(config::get_socket_poll_interval_ms());

    wait::wait_for_socket(&socket_path, timeout, poll_interval)
        .await
        .map_err(CliError::from)
}

/// Spawn the TUI client process.
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

/// Handle the engine stop command.
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
