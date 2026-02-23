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

/// Start the server as a background process.
fn spawn_server() -> Result<(), CliError> {
    let pid_path = config::get_pid_path();

    // Remove stale PID file if it exists
    let _ = daemon::remove_stale_pid(&pid_path);

    // Spawn the server as a background process
    let child = std::process::Command::new("cargo")
        .args(["run", "-p", "chesstty-server"])
        .spawn()
        .map_err(|e| CliError::ProcessError(format!("failed to spawn server: {}", e)))?;

    // Write the PID to the PID file
    let pid = child.id();
    std::fs::write(&pid_path, format!("{}\n", pid))
        .map_err(|e| CliError::ProcessError(format!("failed to write PID file: {}", e)))?;

    Ok(())
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
    let mut child = std::process::Command::new("cargo")
        .args(["run", "-p", "client-tui"])
        .spawn()
        .map_err(|e| CliError::ProcessError(format!("failed to spawn TUI: {}", e)))?;

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
