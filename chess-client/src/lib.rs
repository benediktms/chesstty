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
mod traits;

pub use client::ChessClient;
pub use error::{ClientError, ClientResult};
pub use traits::ChessService;

// Re-export proto types for convenience
pub use chess_proto::*;

// Mock implementation - only available in test mode or with mock feature
#[cfg(any(test, feature = "mock"))]
pub mod mock;
#[cfg(any(test, feature = "mock"))]
pub use mock::{MockCall, MockChessService};
