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
    pub async fn connect(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let channel = Channel::from_shared(addr.to_string())?.connect().await?;

        let client = ChessServiceClient::new(channel);

        Ok(Self {
            client,
            session_id: None,
        })
    }

    /// Create a new game session
    pub async fn create_session(
        &mut self,
        fen: Option<String>,
    ) -> Result<SessionInfo, Box<dyn std::error::Error>> {
        let request = CreateSessionRequest { fen };
        let response = self.client.create_session(request).await?;
        let session_info = response.into_inner();

        // Store the session ID
        self.session_id = Some(session_info.session_id.clone());

        Ok(session_info)
    }

    /// Get current session info
    pub async fn get_session(&mut self) -> Result<SessionInfo, Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

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
    ) -> Result<MakeMoveResponse, Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

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
    ) -> Result<Vec<MoveDetail>, Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        let request = GetLegalMovesRequest {
            session_id: session_id.clone(),
            from_square,
        };

        let response = self.client.get_legal_moves(request).await?;
        Ok(response.into_inner().moves)
    }

    /// Undo the last move
    pub async fn undo_move(&mut self) -> Result<SessionInfo, Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        let request = UndoMoveRequest {
            session_id: session_id.clone(),
        };

        let response = self.client.undo_move(request).await?;
        Ok(response.into_inner())
    }

    /// Reset the game
    pub async fn reset_game(
        &mut self,
        fen: Option<String>,
    ) -> Result<SessionInfo, Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

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
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

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
    pub async fn trigger_engine_move(
        &mut self,
        movetime_ms: Option<u64>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        let request = TriggerEngineMoveRequest {
            session_id: session_id.clone(),
            movetime_ms,
        };

        self.client.trigger_engine_move(request).await?;
        Ok(())
    }

    /// Subscribe to game events (streaming)
    pub async fn stream_events(
        &mut self,
    ) -> Result<tonic::Streaming<GameEvent>, Box<dyn std::error::Error>> {
        let session_id = self.session_id.as_ref().ok_or("No active session")?;

        let request = StreamEventsRequest {
            session_id: session_id.clone(),
        };

        let response = self.client.stream_events(request).await?;
        Ok(response.into_inner())
    }

    /// Close the current session
    pub async fn close_session(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(session_id) = self.session_id.take() {
            let request = CloseSessionRequest { session_id };

            self.client.close_session(request).await?;
        }

        Ok(())
    }
}
