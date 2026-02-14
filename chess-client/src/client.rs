//! Chess gRPC client implementation

use crate::error::{ClientError, ClientResult};
use chess_proto::chess_service_client::ChessServiceClient;
use chess_proto::*;
use tonic::transport::Channel;

/// Network client for communicating with the chess server
pub struct ChessClient {
    client: ChessServiceClient<Channel>,
    session_id: Option<String>,
}

impl ChessClient {
    /// Connect to the chess server
    pub async fn connect(addr: &str) -> ClientResult<Self> {
        let channel = Channel::from_shared(addr.to_string())
            .map_err(|e| ClientError::InvalidAddress(e.to_string()))?
            .connect()
            .await?;

        let client = ChessServiceClient::new(channel);

        Ok(Self {
            client,
            session_id: None,
        })
    }

    /// Create a new game session
    pub async fn create_session(&mut self, fen: Option<String>) -> ClientResult<SessionInfo> {
        let request = CreateSessionRequest { fen };
        let response = self.client.create_session(request).await?;
        let session_info = response.into_inner();

        self.session_id = Some(session_info.session_id.clone());
        Ok(session_info)
    }

    /// Get current session info
    pub async fn get_session(&mut self) -> ClientResult<SessionInfo> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = GetSessionRequest {
            session_id: session_id.clone(),
        };

        let response = self.client.get_session(request).await?;
        Ok(response.into_inner())
    }

    /// Make a move
    pub async fn make_move(
        &mut self,
        from: &str,
        to: &str,
        promotion: Option<String>,
    ) -> ClientResult<MakeMoveResponse> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = MakeMoveRequest {
            session_id: session_id.clone(),
            r#move: Some(MoveRepr {
                from: from.to_string(),
                to: to.to_string(),
                promotion,
            }),
        };

        let response = self.client.make_move(request).await?;
        Ok(response.into_inner())
    }

    /// Get legal moves
    pub async fn get_legal_moves(
        &mut self,
        from_square: Option<String>,
    ) -> ClientResult<Vec<MoveDetail>> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = GetLegalMovesRequest {
            session_id: session_id.clone(),
            from_square,
        };

        let response = self.client.get_legal_moves(request).await?;
        Ok(response.into_inner().moves)
    }

    /// Undo the last move
    pub async fn undo_move(&mut self) -> ClientResult<SessionInfo> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = UndoMoveRequest {
            session_id: session_id.clone(),
        };

        let response = self.client.undo_move(request).await?;
        Ok(response.into_inner())
    }

    /// Reset the game
    pub async fn reset_game(&mut self, fen: Option<String>) -> ClientResult<SessionInfo> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = ResetGameRequest {
            session_id: session_id.clone(),
            fen,
        };

        let response = self.client.reset_game(request).await?;
        Ok(response.into_inner())
    }

    /// Configure the engine
    pub async fn set_engine(
        &mut self,
        enabled: bool,
        skill_level: u32,
        threads: Option<u32>,
        hash_mb: Option<u32>,
    ) -> ClientResult<()> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = SetEngineRequest {
            session_id: session_id.clone(),
            enabled,
            skill_level,
            threads,
            hash_mb,
        };

        self.client.set_engine(request).await?;
        Ok(())
    }

    /// Trigger the engine to make a move
    pub async fn trigger_engine_move(&mut self, movetime_ms: Option<u64>) -> ClientResult<()> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = TriggerEngineMoveRequest {
            session_id: session_id.clone(),
            movetime_ms,
        };

        self.client.trigger_engine_move(request).await?;
        Ok(())
    }

    /// Subscribe to game events (streaming)
    pub async fn stream_events(&mut self) -> ClientResult<tonic::Streaming<GameEvent>> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = StreamEventsRequest {
            session_id: session_id.clone(),
        };

        let response = self.client.stream_events(request).await?;
        Ok(response.into_inner())
    }

    /// Close the current session
    pub async fn close_session(&mut self) -> ClientResult<()> {
        if let Some(session_id) = self.session_id.take() {
            let request = CloseSessionRequest { session_id };
            self.client.close_session(request).await?;
        }
        Ok(())
    }

    /// Suspend the current session
    pub async fn suspend_session(
        &mut self,
        game_mode: &str,
        human_side: Option<&str>,
        skill_level: u32,
    ) -> ClientResult<String> {
        let session_id = self.session_id.as_ref().ok_or(ClientError::NoActiveSession)?;

        let request = SuspendSessionRequest {
            session_id: session_id.clone(),
            game_mode: game_mode.to_string(),
            human_side: human_side.map(|s| s.to_string()),
            skill_level,
        };

        let response = self.client.suspend_session(request).await?;
        self.session_id = None;
        Ok(response.into_inner().suspended_id)
    }

    /// List all suspended sessions
    pub async fn list_suspended_sessions(&mut self) -> ClientResult<Vec<SuspendedSessionInfo>> {
        let request = ListSuspendedSessionsRequest {};
        let response = self.client.list_suspended_sessions(request).await?;
        Ok(response.into_inner().sessions)
    }

    /// Resume a suspended session
    pub async fn resume_suspended_session(&mut self, suspended_id: &str) -> ClientResult<SessionInfo> {
        let request = ResumeSuspendedSessionRequest {
            suspended_id: suspended_id.to_string(),
        };
        let response = self.client.resume_suspended_session(request).await?;
        let info = response.into_inner();
        self.session_id = Some(info.session_id.clone());
        Ok(info)
    }

    /// Delete a suspended session
    pub async fn delete_suspended_session(&mut self, suspended_id: &str) -> ClientResult<()> {
        let request = DeleteSuspendedSessionRequest {
            suspended_id: suspended_id.to_string(),
        };
        self.client.delete_suspended_session(request).await?;
        Ok(())
    }

    /// Save a named position
    pub async fn save_position(&mut self, name: &str, fen: &str) -> ClientResult<String> {
        let request = SavePositionRequest {
            name: name.to_string(),
            fen: fen.to_string(),
        };
        let response = self.client.save_position(request).await?;
        Ok(response.into_inner().position_id)
    }

    /// List all saved positions
    pub async fn list_positions(&mut self) -> ClientResult<Vec<SavedPosition>> {
        let request = ListPositionsRequest {};
        let response = self.client.list_positions(request).await?;
        Ok(response.into_inner().positions)
    }

    /// Delete a saved position
    pub async fn delete_position(&mut self, position_id: &str) -> ClientResult<()> {
        let request = DeletePositionRequest {
            position_id: position_id.to_string(),
        };
        self.client.delete_position(request).await?;
        Ok(())
    }
}
