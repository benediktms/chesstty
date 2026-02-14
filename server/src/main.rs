mod service;
mod session;

use chess_proto::chess_service_server::ChessServiceServer;
use service::ChessServiceImpl;
use session::SessionManager;
use std::sync::Arc;
use tonic::transport::Server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    tracing::info!("Starting ChessTTY gRPC server");

    // Create session manager
    let session_manager = Arc::new(SessionManager::new());

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
