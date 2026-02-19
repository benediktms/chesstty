//! ChessService trait abstraction for client implementations

use crate::error::ClientResult;
use async_trait::async_trait;
use chess_proto::*;

/// Core chess service interface
/// Implemented by both real ChessClient and MockChessService
#[async_trait]
pub trait ChessService: Send + Sync {
    /// Create a new game session
    async fn create_session(
        &mut self,
        fen: Option<String>,
        game_mode: Option<GameModeProto>,
        timer: Option<TimerState>,
    ) -> ClientResult<SessionSnapshot>;

    /// Get current session snapshot
    async fn get_session(&mut self) -> ClientResult<SessionSnapshot>;

    /// Close the active session
    async fn close_session(&mut self) -> ClientResult<()>;

    /// Make a move
    async fn make_move(
        &mut self,
        from: &str,
        to: &str,
        promotion: Option<String>,
    ) -> ClientResult<SessionSnapshot>;

    /// Get legal moves for a square
    async fn get_legal_moves(
        &mut self,
        from_square: Option<String>,
    ) -> ClientResult<Vec<MoveDetail>>;

    /// Pause the current session
    async fn pause_session(&mut self) -> ClientResult<()>;

    /// Resume the current session
    async fn resume_session(&mut self) -> ClientResult<()>;

    /// Configure engine settings
    async fn set_engine(
        &mut self,
        enabled: bool,
        skill_level: u8,
        threads: u32,
        hash_mb: u32,
    ) -> ClientResult<()>;

    /// Stream session events
    async fn stream_session_events(
        &mut self,
    ) -> ClientResult<tonic::Streaming<SessionStreamEvent>>;
}
