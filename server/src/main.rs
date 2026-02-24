mod config;
mod persistence;
mod review;
mod service;
mod session;

use chess_proto::chess_service_server::ChessServiceServer;
use persistence::sqlite::{
    migrate_json_to_sqlite, Database, SqliteAdvancedAnalysisRepository,
    SqliteFinishedGameRepository, SqlitePersistence, SqlitePositionRepository,
    SqliteReviewRepository, SqliteSessionRepository,
};
use service::ChessServiceImpl;
use session::SessionManager;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::signal::unix::{signal, SignalKind};
use tokio_stream::wrappers::UnixListenerStream;
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

    let data_dir = config::get_legacy_data_dir();
    let db_path = config::get_db_path();

    tracing::info!("Using legacy data directory: {}", data_dir.display());
    tracing::info!("Using SQLite database: {}", db_path.display());

    let database = Database::open(&db_path).await?;
    let migration_report = migrate_json_to_sqlite(database.pool(), &data_dir).await?;
    tracing::info!(
        skipped = migration_report.skipped,
        sessions = migration_report.sessions,
        positions = migration_report.positions,
        finished_games = migration_report.finished_games,
        reviews = migration_report.reviews,
        advanced_analyses = migration_report.advanced_analyses,
        "SQLite migration check complete"
    );

    let session_store = SqliteSessionRepository::new(database.pool().clone());
    let position_store = SqlitePositionRepository::new(database.pool().clone());
    let finished_game_store = Arc::new(SqliteFinishedGameRepository::new(database.pool().clone()));
    let review_store = Arc::new(SqliteReviewRepository::new(database.pool().clone()));
    let advanced_store = Arc::new(SqliteAdvancedAnalysisRepository::new(
        database.pool().clone(),
    ));

    // Create session manager
    let session_manager = Arc::new(SessionManager::<SqlitePersistence>::new(
        session_store,
        position_store,
        finished_game_store.clone(),
    ));

    // Create review manager
    let review_manager = Arc::new(review::ReviewManager::<SqlitePersistence>::new(
        finished_game_store,
        review_store,
        advanced_store,
        review::ReviewConfig::default(),
    ));

    // Recover any pending reviews from previous runs
    review_manager.recover_pending_reviews().await;

    // Create service
    let service = ChessServiceImpl::new(session_manager.clone(), review_manager.clone());

    // Server address (Unix Domain Socket)
    let socket_path = config::get_socket_path();

    // Remove stale socket file if it exists
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    let uds = UnixListener::bind(&socket_path)?;
    let uds_stream = UnixListenerStream::new(uds);

    tracing::info!("Server listening on {}", socket_path.display());

    // Set up signal handlers for graceful shutdown
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    // Start server with signal handling
    let server_future = Server::builder()
        .add_service(ChessServiceServer::new(service))
        .serve_with_incoming(uds_stream);

    tokio::select! {
        result = server_future => {
            if let Err(e) = result {
                tracing::error!("Server error: {}", e);
            }
        }
        _ = sigterm.recv() => {
            tracing::info!("Received SIGTERM, shutting down gracefully");
        }
        _ = sigint.recv() => {
            tracing::info!("Received SIGINT, shutting down gracefully");
        }
    }

    // Cleanup socket file
    tracing::info!("Cleaning up...");
    if socket_path.exists() {
        if let Err(e) = std::fs::remove_file(&socket_path) {
            tracing::warn!("Failed to remove socket file: {}", e);
        }
    }
    tracing::info!("Server shut down");

    Ok(())
}
