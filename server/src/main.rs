mod config;
mod persistence;
mod service;
mod session;

use chess_proto::chess_service_server::ChessServiceServer;
use service::ChessServiceImpl;
use session::SessionManager;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing with span durations
    use tracing_subscriber::fmt::format::FmtSpan;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_span_events(FmtSpan::CLOSE)
        .init();

    tracing::info!("Starting ChessTTY gRPC server");

    // Get data and defaults directories
    let data_dir = config::get_data_dir();
    let defaults_dir = Some(config::get_defaults_dir());

    tracing::info!("Using data directory: {}", data_dir.display());
    tracing::info!("Using defaults directory: {:?}", defaults_dir.as_ref().map(|d| d.display()));

    // Create session manager
    let session_manager = Arc::new(SessionManager::new(data_dir, defaults_dir));

    // Create service
    let service = ChessServiceImpl::new(session_manager.clone());

    // Server address
    let addr = "[::1]:50051".parse()?;
    tracing::info!("Server listening on {}", addr);

    // Start server
    Server::builder()
        .add_service(ChessServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}
