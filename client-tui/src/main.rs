mod state;
mod timer;
mod ui;

// Re-export app types for compatibility
pub mod app {
    pub use crate::state::{GameMode, InputPhase, PlayerColor, UciDirection, UciLogEntry, UiState};
}

use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Set up tracing with file output in logs directory
    let log_dir = "logs";
    std::fs::create_dir_all(log_dir).ok();
    let file_appender = tracing_appender::rolling::daily(log_dir, "chesstty-client-tui");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_target(true)
                .with_line_number(true),
        )
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    tracing::info!("ChessTTY Client starting up");

    println!("ChessTTY - Terminal Chess Client");
    println!("Connecting to server at http://[::1]:50051");
    println!();

    // Check for --simple flag to use simplified UI
    let use_simple_ui = std::env::args().any(|arg| arg == "--simple");

    if use_simple_ui {
        println!("Using simplified text-based UI");
        println!("Commands: m <from> <to> | u (undo) | r (reset) | q (quit)");
        println!();
        println!("Debug logs: logs/chesstty-client-tui.YYYY-MM-DD");
        ui::run_simple_app().await?;
    } else {
        println!("ChessTTY - Starting menu...");
        println!("Debug logs: logs/chesstty-client-tui.YYYY-MM-DD");
        ui::run_app().await?;
    }

    tracing::info!("ChessTTY Client shutting down");
    Ok(())
}
