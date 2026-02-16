//! Chess gRPC client library
//!
//! Provides a high-level async client for communicating with chesstty-server.
//! Can be used by TUI, web UI, or any other client application.
//!
//! # Example
//!
//! ```no_run
//! use chess_client::ChessClient;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut client = ChessClient::connect("http://localhost:50051").await?;
//!     let snapshot = client.create_session(None, None, None).await?;
//!     println!("Created session: {}", snapshot.session_id);
//!     Ok(())
//! }
//! ```

mod client;
mod error;

pub use client::ChessClient;
pub use error::{ClientError, ClientResult};

// Re-export proto types for convenience
pub use chess_proto::*;
